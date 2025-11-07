use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    chess::{
        attacks::movegen::{
            gen_capture_promotion_moves, gen_color_moves, is_legal_move, is_square_attacked,
        },
        board::{Board, Color, Piece, STARTPOS_FEN, Square},
        make_move::Undo,
        moves::Move,
    },
    send,
};
use tinyvec::ArrayVec;

#[derive(Clone, Copy, Default, Debug)]
pub struct ClockTime {
    pub white_time_ms: u64,
    pub black_time_ms: u64,
    pub white_increment_ms: u64,
    pub black_increment_ms: u64,
}

#[derive(Clone)]
pub struct TimeControl {
    pub move_time: Option<u64>,
    pub depth: Option<usize>,
    pub clock_time: Option<ClockTime>,
    pub infinite: bool,
    pub start_time: Instant,
}

impl Default for TimeControl {
    fn default() -> Self {
        TimeControl {
            move_time: None,
            depth: None,
            clock_time: None,
            infinite: false,
            start_time: Instant::now(),
        }
    }
}

#[derive(Clone)]
struct PvTable {
    pv: [ArrayVec<[Move; Searcher::MAX_PLY]>; Searcher::MAX_PLY],
}

impl PvTable {
    #[inline(always)]
    fn clear(&mut self, ply: usize) {
        self.pv[ply].clear();
    }

    #[inline(always)]
    fn update(&mut self, ply: usize, mov: Move) {
        // set the best move for this ply
        self.pv[ply].clear();
        self.pv[ply].push(mov);

        // copy child PV (if any)
        if ply + 1 < Searcher::MAX_PLY && !self.pv[ply + 1].is_empty() {
            let (left, right) = self.pv.split_at_mut(ply + 1);
            let curr = &mut left[ply];
            let next = &right[0];
            curr.extend(next.iter().copied());
        }
    }

    #[inline(always)]
    fn get(&self, ply: usize) -> &[Move] {
        &self.pv[ply]
    }
}

#[derive(Clone)]
pub struct Searcher {
    // core search
    board: Board,
    history: ArrayVec<[u64; 1024]>,
    pv_table: PvTable,

    // search info tracking
    nodes: usize,
    seldepth: usize,

    // timing
    pub time_control: TimeControl,
    time_ms: Option<u64>,
    was_pondering: bool,

    // external control
    stop: Arc<AtomicBool>,
    ponder: Arc<AtomicBool>,
}

impl Searcher {
    const MAX_PLY: usize = 64;
    const CHECKMATE_SCORE: i16 = 30_000;
    const CHECKMATE_THRESHOLD: i16 = Searcher::CHECKMATE_SCORE - 2 * Searcher::MAX_PLY as i16;
    const NODE_MASK: usize = 1023;
    const INF: i16 = 32_000;

    #[inline(always)]
    fn is_three_fold_repetition(&self) -> bool {
        self.history
            .iter()
            .rev()
            .step_by(2) // check only positions with same side to move
            .take(self.board.halfmove_clock as usize / 2)
            .filter(|&&zobrist| zobrist == self.board.zobrist)
            .take(2)
            .count()
            >= 2
    }

    #[inline(always)]
    fn is_draw(&self) -> bool {
        self.board.is_fifty_move()
            || self.is_three_fold_repetition()
            || self.board.is_insufficient_material()
    }

    #[inline(always)]
    fn push_move(&mut self, mov: Move) -> Undo {
        self.history.push(self.board.zobrist);
        self.board.make_move(mov)
    }

    #[inline(always)]
    fn pop_move(&mut self, undo: &Undo) {
        self.board.undo_move(undo);
        self.history.pop();
    }

    fn calculate_time_from_clock(color: Color, clock_time: &ClockTime) -> u64 {
        let color_time_ms = match color {
            Color::White => clock_time.white_time_ms,
            Color::Black => clock_time.black_time_ms,
        };
        let color_increment_ms = match color {
            Color::White => clock_time.white_increment_ms,
            Color::Black => clock_time.black_increment_ms,
        };

        color_time_ms / 20 + color_increment_ms / 2
    }

    pub fn start_search(&mut self) -> (Move, Option<Move>) {
        let control = &self.time_control;

        // guard stop flag
        self.stop.store(false, Ordering::Relaxed);

        // initialize worker-local timing state according to mode
        if self.ponder.load(Ordering::Relaxed) {
            // if starting in ponder, compute the time we'd use *after* ponderhit but
            // DO NOT reset start_time here — the worker must wait for ponderhit to start clock.
            self.time_ms = control
                .clock_time
                .as_ref()
                .map(|ct| Searcher::calculate_time_from_clock(self.board.side_to_move, ct))
                .or(control.move_time);
            self.was_pondering = true;
        } else if let Some(mt) = control.move_time {
            self.time_ms = Some(mt);
            self.was_pondering = false;
        } else if control.depth.is_some() {
            self.time_ms = None;
            self.was_pondering = false;
        } else if let Some(clock_time) = &control.clock_time {
            self.time_ms = Some(Searcher::calculate_time_from_clock(
                self.board.side_to_move,
                clock_time,
            ));
            self.was_pondering = false;
        } else if control.infinite {
            self.time_ms = None;
            self.was_pondering = false;
        } else {
            self.time_ms = None;
            self.was_pondering = false;
        }

        let (best_move, ponder_move) = if self.was_pondering {
            // when pondering, iterative_deepening runs and will await ponderhit
            self.iterative_deepening(None)
        } else if control.depth.is_some() {
            self.iterative_deepening(control.depth)
        } else {
            self.iterative_deepening(None)
        };

        // ensure local ponder flag cleared (the Arc is shared across clones)
        self.ponder.store(false, Ordering::Relaxed);

        (best_move, ponder_move)
    }

    #[inline(always)]
    fn time_to_stop(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
            || self.time_ms.is_some_and(|ms| {
                !self.ponder.load(Ordering::Relaxed)
                    && !self.was_pondering
                    && self.time_control.start_time.elapsed().as_millis() >= ms as u128
            })
    }

    fn print_info(&self, searching_time: Duration, best_score: i16, current_depth: usize) {
        let score_str = if best_score.abs() >= Searcher::CHECKMATE_THRESHOLD {
            // get the mate distance and convert to full moves
            let mate_in = (Searcher::CHECKMATE_SCORE - best_score.abs() + 1) / 2;
            let mate_in = if best_score > 0 { mate_in } else { -mate_in };

            format!("mate {}", mate_in)
        } else {
            format!("cp {}", best_score)
        };
        let nps = self.nodes as f64 / searching_time.as_secs_f64();
        let pv_line = self.pv_table.get(0);
        let searching_time_ms = searching_time.as_millis();

        send!(
            "info depth {} seldepth {} score {} nodes {} nps {} time {} pv {}",
            current_depth,
            self.seldepth,
            score_str,
            self.nodes,
            nps as u64,
            if searching_time_ms == 0 {
                1
            } else {
                searching_time_ms
            },
            pv_line
                .iter()
                .take(current_depth)
                .map(|mov| mov.to_uci())
                .reduce(|a, b| format!("{a} {b}"))
                .unwrap_or_default()
        );
    }

    fn iterative_deepening(&mut self, depth: Option<usize>) -> (Move, Option<Move>) {
        let move_list = gen_color_moves(&self.board);
        let mut best_move: Move = move_list[0];
        let mut current_depth = 1;
        let mut ponder_move: Option<Move> = None;

        loop {
            let (mut alpha, beta) = (-Searcher::INF, Searcher::INF);

            let mut step_best_move = best_move;
            let mut best_score = -Searcher::INF;
            let mut last_info_time = Duration::ZERO;

            for (number, &mov) in move_list.iter().enumerate() {
                // if we started in ponder mode and now ponder has been turned off, initialize timing here:
                if self.was_pondering && !self.ponder.load(Ordering::Relaxed) {
                    // reset the worker's clock now that GUI said ponderhit
                    self.time_control.start_time = Instant::now();

                    // recompute time_ms from the worker's own time_control (it may be Option)
                    if let Some(clock_time) = self.time_control.clock_time {
                        self.time_ms = Some(Searcher::calculate_time_from_clock(
                            self.board.side_to_move,
                            &clock_time,
                        ));
                    }
                    // if it was a movetime control, self.time_ms was already set in start_search
                    // otherwise leave None for infinite

                    self.was_pondering = false;

                    continue;
                }

                // inline manually to avoid calling `.elapsed()` twice
                let elapsed = self.time_control.start_time.elapsed();

                if self.stop.load(Ordering::Relaxed)
                    || self.time_ms.is_some_and(|ms| {
                        !self.ponder.load(Ordering::Relaxed) && elapsed.as_millis() >= ms as u128
                    })
                {
                    break;
                }

                if elapsed - last_info_time >= Duration::from_secs(1) {
                    send!(
                        "info depth {current_depth} currmove {} currmovenumber {}",
                        mov.to_uci(),
                        number + 1,
                    );
                    last_info_time = elapsed;
                }

                let undo = self.push_move(mov);
                if is_legal_move(mov, &self.board) {
                    let score = -self.search(-beta, -alpha, current_depth - 1, 1);

                    if score > best_score {
                        step_best_move = mov;
                        best_score = score;
                        self.pv_table.update(0, mov);
                    }
                    if score > alpha {
                        alpha = score;
                    }
                    if alpha >= beta {
                        self.pop_move(&undo);
                        break;
                    }
                }
                self.pop_move(&undo);
            }

            if self.time_to_stop() {
                break;
            }

            let searching_time = self.time_control.start_time.elapsed();

            best_move = step_best_move;
            ponder_move = self.pv_table.get(0).get(1).cloned();
            self.print_info(searching_time, best_score, current_depth);

            if current_depth >= depth.unwrap_or(Searcher::MAX_PLY) {
                break;
            }

            current_depth += 1;
            self.nodes = 0;
        }

        (best_move, ponder_move)
    }

    /// draw score formula
    #[inline(always)]
    fn get_draw_score(eval: i16) -> i16 {
        (-eval / 10).clamp(-100, 100)
    }

    /// in centipawn
    fn search(&mut self, mut alpha: i16, beta: i16, depth: usize, ply: usize) -> i16 {
        if depth == 0 {
            return self.quiescence(alpha, beta, ply);
        }

        self.nodes += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        let check_timeout = self.nodes & Searcher::NODE_MASK == 0;
        let color = self.board.side_to_move;
        let static_eval = match color {
            Color::White => self.board.evaluate(),
            Color::Black => -self.board.evaluate(),
        };

        if self.is_draw() {
            self.pv_table.clear(ply);
            return Searcher::get_draw_score(static_eval);
        }

        if check_timeout && self.time_to_stop() {
            return static_eval;
        }

        let mate_score = Searcher::CHECKMATE_SCORE - ply as i16;
        let mut best_score = -Searcher::INF;
        let mut found_legal_move = false;

        for mov in gen_color_moves(&self.board) {
            let undo = self.push_move(mov);
            if !is_legal_move(mov, &self.board) {
                self.pop_move(&undo);
                continue;
            }

            found_legal_move = true;
            let score = -self.search(-beta, -alpha, depth - 1, ply + 1);
            self.pop_move(&undo);

            if score > best_score {
                best_score = score;
                self.pv_table.update(ply, mov);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                return alpha;
            }
            if check_timeout && self.time_to_stop() {
                return alpha;
            }
        }

        if found_legal_move {
            best_score
        } else {
            self.pv_table.clear(ply);
            let king_square = self.board.bitboards[color as usize][Piece::King as usize]
                .trailing_zeros() as Square;
            let in_check = is_square_attacked(king_square, color.toggle(), &self.board);

            if in_check {
                -mate_score
            } else {
                Searcher::get_draw_score(static_eval) // stalemate
            }
        }
    }

    fn quiescence(&mut self, mut alpha: i16, beta: i16, ply: usize) -> i16 {
        self.nodes += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        let check_timeout = self.nodes & Searcher::NODE_MASK == 0;
        let color = self.board.side_to_move;
        let static_eval = match color {
            Color::White => self.board.evaluate(),
            Color::Black => -self.board.evaluate(),
        };

        if self.is_draw() {
            return Searcher::get_draw_score(static_eval);
        }

        if (check_timeout && self.time_to_stop()) || ply >= Searcher::MAX_PLY {
            return static_eval;
        }

        // stand-pat score
        let mut best_score = static_eval;

        // stand-pat cutoff
        if best_score >= beta {
            return best_score;
        }
        if best_score > alpha {
            alpha = best_score;
        }

        // delta pruning
        const DELTA_MARGIN: i16 = 75;
        if best_score + Board::PIECE_VALUES[Piece::Queen as usize] + DELTA_MARGIN < alpha {
            return alpha;
        }

        let mate_score = Searcher::CHECKMATE_SCORE - ply as i16;
        let in_check = is_square_attacked(
            self.board.bitboards[color as usize][Piece::King as usize].trailing_zeros() as Square,
            color.toggle(),
            &self.board,
        );

        // if in check we must generate /all/ evasions (not only captures)
        let move_list = if in_check {
            gen_color_moves(&self.board) // every legal move is an evasion
        } else {
            gen_capture_promotion_moves(&self.board)
        };

        let mut found_legal_move = false;
        for mov in move_list {
            let undo = self.push_move(mov);
            if !is_legal_move(mov, &self.board) {
                self.pop_move(&undo);
                continue;
            }

            found_legal_move = true;
            let score = -self.quiescence(-beta, -alpha, ply + 1);
            self.pop_move(&undo);

            if score > best_score {
                best_score = score;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta || (check_timeout && self.time_to_stop()) {
                return alpha;
            }
        }

        if found_legal_move {
            best_score
        } else {
            if in_check {
                -mate_score
            } else {
                best_score // if no captures found: stand-pat
            }
        }
    }

    pub fn new() -> Searcher {
        Searcher {
            board: Board::new(STARTPOS_FEN).unwrap(),
            history: ArrayVec::new(),
            pv_table: PvTable {
                pv: [ArrayVec::new(); Searcher::MAX_PLY],
            },

            nodes: 0,
            seldepth: 0,

            time_control: TimeControl::default(),
            time_ms: None,
            was_pondering: false,

            stop: Arc::new(AtomicBool::new(false)),
            ponder: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline(always)]
    pub fn start_pondering(&mut self) {
        self.ponder.store(true, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn stop_pondering(&mut self) {
        self.ponder.store(false, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn copy_pos(&mut self, board: &Board, history: &ArrayVec<[u64; 1024]>) {
        self.board = board.clone();
        self.history = *history;
    }

    #[inline(always)]
    pub fn stop_search(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
