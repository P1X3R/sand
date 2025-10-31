use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    chess::{
        attacks::movegen::{gen_color_moves, is_legal_move, is_square_attacked},
        board::{Board, Color, Piece, STARTPOS_FEN, Square},
        make_move::Undo,
        moves::Move,
    },
    send,
};
use tinyvec::ArrayVec;

#[derive(Clone, Copy, Default)]
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
    pub ponder: bool,
    pub start_time: Instant,
}

impl Default for TimeControl {
    fn default() -> Self {
        TimeControl {
            move_time: None,
            depth: None,
            clock_time: None,
            infinite: false,
            ponder: false,
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
    board: Board,
    history: ArrayVec<[u64; 1024]>, // For three-fold repetition
    pub time_control: TimeControl,
    stop: Arc<AtomicBool>,
    pv_table: PvTable,
    nodes: usize,
}

impl Searcher {
    const MAX_PLY: usize = 64;
    const CHECKMATE_SCORE: i16 = 20_000;
    const CHECKMATE_THRESHOLD: i16 = Searcher::CHECKMATE_SCORE - Searcher::MAX_PLY as i16 - 1;
    const INF: i16 = 30_000;
    const DRAW_SCORE: i16 = 0;

    #[inline(always)]
    fn is_three_fold_repetition(&self) -> bool {
        self.history
            .iter()
            .rev()
            .skip(2) // skip current position
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
        let undo = self.board.make_move(mov);
        self.history.push(self.board.zobrist);

        undo
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

    pub fn start_search(&mut self) -> Move {
        let control = &self.time_control;

        // Guard stop flag
        self.stop.store(false, Ordering::Relaxed);

        if control.ponder {
            // Set to 0 ms by default to automatically stop once `ponderhit` if GUI sent just:
            // `go ponder` with no time control
            let time_ms = control.clock_time.as_ref().map_or(0, |clock_time| {
                Searcher::calculate_time_from_clock(self.board.side_to_move, clock_time)
            });

            return self.iterative_deepening(Some(time_ms), None);
        } else if let Some(move_time) = control.move_time {
            return self.iterative_deepening(Some(move_time), None);
        } else if let Some(depth) = control.depth {
            return self.iterative_deepening(None, Some(depth));
        } else if let Some(clock_time) = &control.clock_time {
            return self.iterative_deepening(
                Some(Searcher::calculate_time_from_clock(
                    self.board.side_to_move,
                    clock_time,
                )),
                None,
            );
        } else if control.infinite {
            return self.iterative_deepening(None, None);
        }

        self.iterative_deepening(None, Some(1)) // Dummy and fast search by default
    }

    #[inline(always)]
    fn time_to_stop(&self, time_ms: Option<u64>) -> bool {
        self.stop.load(Ordering::Relaxed)
            || time_ms.is_some_and(|ms| {
                !self.time_control.ponder
                    && self.time_control.start_time.elapsed().as_millis() >= ms as u128
            })
    }

    fn print_info(&self, searching_time: Duration, best_score: i16, current_depth: usize) {
        let score_str = if best_score.abs() >= Searcher::CHECKMATE_THRESHOLD {
            // Get the mate distance and convert to full moves
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
            "info depth {current_depth} score {score_str} nodes {} nps {} time {} pv {}",
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

    fn iterative_deepening(&mut self, time_ms: Option<u64>, depth: Option<usize>) -> Move {
        let move_list = gen_color_moves(&self.board);
        let mut best_move: Move = move_list[0];
        let mut current_depth = 1;

        loop {
            let (mut alpha, beta) = (-Searcher::INF, Searcher::INF);

            let mut step_best_move = best_move;
            let mut best_score = -Searcher::INF;
            let mut last_info_time = Duration::ZERO;

            for (number, &mov) in move_list.iter().enumerate() {
                // Inline manually to avoid calling `.elapsed()` twice
                let elapsed = self.time_control.start_time.elapsed();

                if self.stop.load(Ordering::Relaxed)
                    || time_ms.is_some_and(|ms| {
                        !self.time_control.ponder && elapsed.as_millis() >= ms as u128
                    })
                {
                    break;
                }

                if elapsed.as_millis() - last_info_time.as_millis() >= 1000 {
                    send!(
                        "info depth {current_depth} currmove {} currmovenumber {}",
                        mov.to_uci(),
                        number + 1,
                    );
                    last_info_time = elapsed;
                }

                let undo = self.push_move(mov);
                if is_legal_move(mov, &self.board) {
                    let score = -self.search(-beta, -alpha, current_depth - 1, 1, time_ms);
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
            let searching_time = self.time_control.start_time.elapsed();

            if self.time_to_stop(time_ms) || current_depth >= depth.unwrap_or(Searcher::MAX_PLY) {
                break;
            }

            best_move = step_best_move;
            self.print_info(searching_time, best_score, current_depth);

            current_depth += 1;
            self.nodes = 0;
        }

        best_move
    }

    /// In centipawn
    fn search(
        &mut self,
        mut alpha: i16,
        beta: i16,
        depth: usize,
        ply: usize,
        time_ms: Option<u64>,
    ) -> i16 {
        const NODE_MASK: usize = 1023;

        self.nodes += 1;
        let color = self.board.side_to_move;

        if self.is_draw() {
            self.pv_table.clear(ply);
            return Searcher::DRAW_SCORE;
        }
        if depth == 0 || (self.nodes & NODE_MASK == 0 && self.time_to_stop(time_ms)) {
            return match color {
                Color::White => self.board.evaluate(),
                Color::Black => -self.board.evaluate(),
            };
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
            let score = -self.search(-beta, -alpha, depth - 1, ply + 1, time_ms);

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
            if self.nodes & NODE_MASK == 0 && self.time_to_stop(time_ms) {
                return alpha;
            }
        }

        if !found_legal_move {
            self.pv_table.clear(ply);
            let in_check = is_square_attacked(
                self.board.bitboards[color as usize][Piece::King as usize].trailing_zeros()
                    as Square,
                color.toggle(),
                &self.board,
            );

            return if in_check {
                -mate_score
            } else {
                Searcher::DRAW_SCORE
            };
        }

        best_score
    }

    pub fn new() -> Searcher {
        Searcher {
            board: Board::new(STARTPOS_FEN).unwrap(),
            history: ArrayVec::new(),
            time_control: TimeControl::default(),
            stop: Arc::from(AtomicBool::new(false)),
            pv_table: PvTable {
                pv: [ArrayVec::new(); Searcher::MAX_PLY],
            },
            nodes: 0,
        }
    }

    pub fn copy_pos(&mut self, board: &Board, history: &ArrayVec<[u64; 1024]>) {
        self.board = board.clone();
        self.history = *history;
    }

    pub fn stop_search(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
