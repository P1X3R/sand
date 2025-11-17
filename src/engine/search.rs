use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    chess::*,
    engine::{
        ordering::*,
        transposition::{Bound, TT},
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
pub enum TimeControl {
    MoveTime(u64),
    Depth(usize),
    ClockTime(ClockTime),
    Infinite,
}

type PvLine = ArrayVec<[Move; Searcher::MAX_PLY]>;

#[derive(Clone)]
struct PvTable {
    pv: [PvLine; Searcher::MAX_PLY],
}

impl PvTable {
    fn clear(&mut self, ply: usize) {
        self.pv[ply].clear();
    }

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

    fn get(&self, ply: usize) -> &[Move] {
        &self.pv[ply]
    }
}

#[derive(Clone, Copy)]
struct TimeManagement {
    hard_limit: Duration, // absolute maximum
    soft_limit: Duration, // target to finish by
    cached_elapsed: Duration,
    elapsed_clock: usize,
    start: Instant,
}

impl TimeManagement {
    const TIME_CHECKPOINT: usize = 1023;

    pub fn from_clock(color: Color, clock_time: &ClockTime) -> TimeManagement {
        let color_time_ms = match color {
            Color::White => clock_time.white_time_ms,
            Color::Black => clock_time.black_time_ms,
        };
        let color_increment_ms = match color {
            Color::White => clock_time.white_increment_ms,
            Color::Black => clock_time.black_increment_ms,
        };

        // https://www.chessprogramming.org/Time_Management#Basic_TM
        let base_time = color_time_ms / 20 + color_increment_ms / 2;

        TimeManagement {
            // 20% more of the time to exit
            hard_limit: Duration::from_millis(
                (base_time as f64 * 1.2).min(color_time_ms as f64) as u64
            ),

            // 80% of the base time to think
            soft_limit: Duration::from_millis((base_time as f64 * 0.8) as u64),

            elapsed_clock: 0,
            cached_elapsed: Duration::ZERO,

            start: Instant::now(),
        }
    }

    pub fn from_millis(millis: u64) -> TimeManagement {
        TimeManagement {
            hard_limit: Duration::from_millis(millis),
            soft_limit: Duration::from_millis((millis as f64 * 0.8) as u64),

            elapsed_clock: 0,
            cached_elapsed: Duration::ZERO,

            start: Instant::now(),
        }
    }

    pub fn is_timeout(&mut self, is_depth_complete: bool) -> bool {
        self.elapsed_clock += 1;
        if self.elapsed_clock >= Self::TIME_CHECKPOINT {
            self.cached_elapsed = self.start.elapsed();
            self.elapsed_clock = 0;
        }

        self.cached_elapsed >= self.hard_limit
            || (self.cached_elapsed >= self.soft_limit && is_depth_complete)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
pub enum SearchMode {
    Normal = 0,
    Ponder = 1,

    // we were pondering and now got a move. Changes instantly to `Normal`
    PonderHit = 2,

    Stop = 3,
}

#[derive(Debug)]
pub struct AtomicSearchMode {
    inner: AtomicU8,
}

impl AtomicSearchMode {
    pub fn new(mode: SearchMode) -> Self {
        Self {
            inner: AtomicU8::new(mode as u8),
        }
    }

    fn from_u8(number: u8) -> SearchMode {
        match number {
            0 => SearchMode::Normal,
            1 => SearchMode::Ponder,
            2 => SearchMode::PonderHit,
            3 => SearchMode::Stop,
            _ => unreachable!(),
        }
    }

    pub fn load(&self) -> SearchMode {
        Self::from_u8(self.inner.load(Ordering::Relaxed))
    }

    pub fn store(&self, mode: SearchMode) {
        self.inner.store(mode as u8, Ordering::Relaxed);
    }
}

pub type ZobristHistory = ArrayVec<[u64; 1024]>;

pub struct Searcher {
    board: Board,
    history: ZobristHistory,
    pv_table: PvTable,
    prev_pv: PvLine,

    nodes: usize,
    seldepth: usize,

    time: Option<TimeManagement>,
    time_control: TimeControl,

    search_mode: Arc<AtomicSearchMode>,

    killers: [[Option<Move>; 2]; Searcher::MAX_PLY],
    history_heuristic: Arc<HistoryHeuristics>,
    age: u8,
    tt: Arc<TT>,
}

impl Searcher {
    pub const MAX_PLY: usize = 64;
    const CHECKMATE_SCORE: i16 = 30_000;
    pub const CHECKMATE_THRESHOLD: i16 = Searcher::CHECKMATE_SCORE - 2 * Searcher::MAX_PLY as i16;
    pub const INF: i16 = 32_000;

    fn is_three_fold_repetition(&self) -> bool {
        self.history
            .iter()
            .rev()
            .take(self.board.halfmove_clock as usize)
            .step_by(2) // check only positions with same side to move
            .filter(|&&zobrist| zobrist == self.board.zobrist)
            .take(2)
            .count()
            >= 2
    }

    fn is_draw(&self) -> bool {
        self.board.is_fifty_move()
            || self.is_three_fold_repetition()
            || self.board.is_insufficient_material()
    }

    fn push_move(&mut self, mov: Move) -> Undo {
        let undo = self.board.make_move(mov);
        self.history.push(self.board.zobrist);

        undo
    }

    fn pop_move(&mut self, undo: &Undo) {
        self.board.undo_move(undo);
        self.history.pop();
    }

    pub fn start_search(&mut self, control: TimeControl) -> (Move, Option<Move>) {
        self.time_control = control.clone();
        self.time = match control {
            TimeControl::ClockTime(ct) => {
                Some(TimeManagement::from_clock(self.board.side_to_move, &ct))
            }
            TimeControl::MoveTime(mt) => Some(TimeManagement::from_millis(mt)),
            _ => None,
        };

        let depth = if let TimeControl::Depth(d) = control {
            Some(d)
        } else {
            None
        };

        let (best_move, ponder_move) = self.iterative_deepening(depth);

        // guard stop flag
        self.search_mode.store(SearchMode::Normal);

        (best_move, ponder_move)
    }

    fn time_to_stop(&mut self, is_depth_complete: bool) -> bool {
        let search_mode = self.search_mode.load();

        if search_mode == SearchMode::PonderHit {
            self.time = match self.time_control {
                TimeControl::ClockTime(ct) => {
                    Some(TimeManagement::from_clock(self.board.side_to_move, &ct))
                }
                TimeControl::MoveTime(mt) => Some(TimeManagement::from_millis(mt)),
                TimeControl::Infinite | TimeControl::Depth(_) => None, // no time limit
            };

            self.search_mode.store(SearchMode::Normal);
            return false;
        }

        search_mode == SearchMode::Stop
            || (search_mode != SearchMode::Ponder
                && self
                    .time
                    .as_mut()
                    .is_some_and(|t| t.is_timeout(is_depth_complete)))
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
            "info depth {} seldepth {} score {} nodes {} nps {} time {} hashfull {} pv {}",
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
            self.tt.get_hashfull(),
            pv_line
                .iter()
                .take(current_depth)
                .map(|mov| mov.to_uci())
                .reduce(|a, b| format!("{a} {b}"))
                .unwrap_or_default()
        );
    }

    /// this function updates killer moves and history heuristics on beta cut-off
    fn update_heuristics(
        &mut self,
        depth: usize,
        ply: usize,
        mov: Move,
        scored_list: &ScoredMoveList,
        move_index: usize,
    ) {
        let move_type = mov.get_flags().move_type;
        if move_type != MoveType::Capture && move_type != MoveType::EnPassantCapture {
            if self.killers[ply][0] != Some(mov) {
                self.killers[ply][1] = self.killers[ply][0];
                self.killers[ply][0] = Some(mov);
            }

            let bonus = (depth * depth) as i32;
            let color = self.board.side_to_move;

            self.history_heuristic
                .update(color, mov.get_from(), mov.get_to(), bonus);

            // apply history maluses
            // this works becasue the `scored_iter` orders the already seen moves behind
            // `move_index`, so iterate from 0 to the current one is essentially iterate over the
            // already seen moves
            for (quiet_move, _) in scored_list.iter().take(move_index) {
                let quiet_move_type = quiet_move.get_flags().move_type;

                if quiet_move_type == MoveType::Capture
                    || quiet_move_type == MoveType::EnPassantCapture
                {
                    continue;
                }

                self.history_heuristic.update(
                    color,
                    quiet_move.get_from(),
                    quiet_move.get_to(),
                    -bonus,
                );
            }
        }
    }

    fn iterative_deepening(&mut self, depth: Option<usize>) -> (Move, Option<Move>) {
        let move_list = gen_color_moves(&self.board);
        let mut best_move: Move = move_list[0];
        let mut current_depth = 1;
        let mut ponder_move: Option<Move> = None;
        let search_start = Instant::now(); // used only for `info` updates

        self.tt.reset_used_counter();

        loop {
            let (mut alpha, beta) = (-Searcher::INF, Searcher::INF);

            let mut step_best_move = best_move;
            let mut best_score = -Searcher::INF;
            let mut last_info_time = Duration::ZERO;

            let mut scored_moves = score(&move_list, &self.ctx(0, None));
            for (move_index, mov) in scored_moves.scored_iter().enumerate() {
                if current_depth > 1 && self.time_to_stop(false) {
                    break;
                }

                let elapsed = search_start.elapsed();
                if elapsed
                    .checked_sub(last_info_time)
                    .is_some_and(|diff| diff >= Duration::from_secs(1))
                {
                    send!(
                        "info depth {current_depth} currmove {} currmovenumber {}",
                        mov.to_uci(),
                        move_index + 1,
                    );
                    last_info_time = elapsed;
                }

                let undo = self.push_move(mov);
                if is_legal_move(mov, &self.board) {
                    // the side to move is not toggled here because it's already toggled by
                    // `push_move`
                    let gives_check = is_king_attcked(self.board.side_to_move, &self.board);

                    let score = -self.search(-beta, -alpha, current_depth - 1, 1, gives_check);

                    if score > best_score {
                        step_best_move = mov;
                        best_score = score;
                        self.pv_table.update(0, mov);
                    }
                    if score > alpha {
                        alpha = score;
                    }
                    if alpha >= beta {
                        self.update_heuristics(0, current_depth, mov, &scored_moves, move_index);
                        break;
                    }
                }
                self.pop_move(&undo);
            }

            let searching_time = search_start.elapsed();

            if self.time_to_stop(true) {
                if current_depth <= 1 {
                    self.print_info(searching_time, best_score, current_depth);
                    best_move = step_best_move;
                }
                break;
            }

            let pv_line = self.pv_table.get(0);

            best_move = step_best_move;
            ponder_move = pv_line.get(1).cloned();
            self.print_info(searching_time, best_score, current_depth);
            self.prev_pv = pv_line.try_into().unwrap_or_default();

            if current_depth >= depth.unwrap_or(Searcher::MAX_PLY) {
                break;
            }

            current_depth += 1;
            self.nodes = 0;
        }

        (best_move, ponder_move)
    }

    fn get_draw_score(eval: i16) -> i16 {
        (-eval / 10).clamp(-100, 100)
    }

    fn mate_distance_pruning(ply: usize, alpha: i16, beta: i16) -> Option<i16> {
        let max_mate = Searcher::CHECKMATE_SCORE - ply as i16;
        let min_mate = -Searcher::CHECKMATE_SCORE + ply as i16;

        if alpha >= max_mate {
            return Some(alpha);
        }
        if beta <= min_mate {
            return Some(beta);
        }

        None
    }

    /// in centipawn
    fn search(
        &mut self,
        mut alpha: i16,
        beta: i16,
        depth: usize,
        ply: usize,
        in_check: bool,
    ) -> i16 {
        if depth == 0 {
            return self.quiescence(alpha, beta, ply, in_check);
        }

        self.nodes += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        if let Some(score) = Searcher::mate_distance_pruning(ply, alpha, beta) {
            return score;
        }

        let entry = self.tt.probe(self.board.zobrist, depth);
        let hash_move = entry.and_then(|e| Some(e.best_move));

        if let Some(e) = entry
            && let Some(entry_score) = e.probe(alpha, beta, ply)
        {
            return entry_score;
        }

        let color = self.board.side_to_move;
        let max_mate = Searcher::CHECKMATE_SCORE - ply as i16;
        let static_eval = match color {
            Color::White => self.board.evaluate(),
            Color::Black => -self.board.evaluate(),
        };

        if self.is_draw() {
            self.pv_table.clear(ply);
            return Searcher::get_draw_score(static_eval);
        }

        let mut best_score = -Searcher::INF;
        let mut found_legal_move = false;

        let mut scored_moves = score(&gen_color_moves(&self.board), &self.ctx(ply, hash_move));
        for (move_index, mov) in scored_moves.scored_iter().enumerate() {
            let undo = self.push_move(mov);
            if !is_legal_move(mov, &self.board) {
                self.pop_move(&undo);
                continue;
            }

            found_legal_move = true;
            let gives_check = is_king_attcked(self.board.side_to_move, &self.board);
            let score = -self.search(-beta, -alpha, depth - 1, ply + 1, gives_check);
            self.pop_move(&undo);

            if score > best_score {
                best_score = score;
                self.pv_table.update(ply, mov);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                self.update_heuristics(ply, depth, mov, &scored_moves, move_index);
                return alpha;
            }
            if self.time_to_stop(false) {
                return alpha;
            }
        }

        if found_legal_move {
            self.tt.store(
                self.board.zobrist,
                depth,
                best_score,
                self.pv_table.get(ply)[0],
                Bound::from_score(best_score, alpha, beta),
                self.age,
                ply,
            );

            best_score
        } else {
            self.pv_table.clear(ply);

            if in_check {
                -max_mate
            } else {
                Searcher::get_draw_score(static_eval) // stalemate
            }
        }
    }

    fn quiescence(&mut self, mut alpha: i16, beta: i16, ply: usize, in_check: bool) -> i16 {
        self.nodes += 1;
        if ply > self.seldepth {
            self.seldepth = ply;
        }

        if let Some(score) = Searcher::mate_distance_pruning(ply, alpha, beta) {
            return score;
        }

        let entry = self.tt.probe(self.board.zobrist, 0);
        let hash_move = entry.and_then(|e| Some(e.best_move));

        if let Some(e) = entry
            && let Some(entry_score) = e.probe(alpha, beta, ply)
        {
            return entry_score;
        }

        let color = self.board.side_to_move;
        let static_eval = match color {
            Color::White => self.board.evaluate(),
            Color::Black => -self.board.evaluate(),
        };

        if self.is_draw() {
            return Searcher::get_draw_score(static_eval);
        }

        if self.time_to_stop(false) || ply >= Searcher::MAX_PLY {
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
        if static_eval + Board::PIECE_VALUES[Piece::Queen as usize] + DELTA_MARGIN < alpha {
            return alpha;
        }

        let max_mate = Searcher::CHECKMATE_SCORE - ply as i16;

        // if in check we must generate all evasions (not only captures)
        let move_list = if in_check {
            gen_color_moves(&self.board) // every legal move is an evasion
        } else {
            gen_capture_promotion_moves(&self.board)
        };

        // null move, sentinel is intentional, used only to store in TT
        let mut best_move: Move = Move(0);

        let mut found_legal_move = false;
        for mov in score(&move_list, &self.ctx(ply, hash_move)).scored_iter() {
            let can_prune = !in_check && can_prune_by_see(mov, &self.board);

            let undo = self.push_move(mov);
            if !is_legal_move(mov, &self.board) {
                self.pop_move(&undo);
                continue;
            }

            let gives_check = is_king_attcked(self.board.side_to_move, &self.board);

            found_legal_move = true;
            if can_prune && !gives_check {
                self.pop_move(&undo);
                continue;
            }

            let score = -self.quiescence(-beta, -alpha, ply + 1, gives_check);
            self.pop_move(&undo);

            if score > best_score {
                best_score = score;
                best_move = mov;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta || self.time_to_stop(false) {
                return alpha;
            }
        }

        if found_legal_move {
            self.tt.store(
                self.board.zobrist,
                0,
                best_score,
                best_move,
                Bound::from_score(best_score, alpha, beta),
                self.age,
                ply,
            );

            best_score
        } else {
            if in_check {
                -max_mate
            } else {
                best_score // if no captures found: stand-pat
            }
        }
    }

    pub fn new(
        board: Board,
        history: ZobristHistory,
        search_mode: &Arc<AtomicSearchMode>,
        history_heuristic: &Arc<HistoryHeuristics>,
        age: u8,
        tt: &Arc<TT>,
    ) -> Searcher {
        Searcher {
            board,
            history,
            pv_table: PvTable {
                pv: [PvLine::new(); Searcher::MAX_PLY],
            },
            prev_pv: PvLine::new(),

            nodes: 0,
            seldepth: 0,

            time: None,
            time_control: TimeControl::Infinite,
            search_mode: Arc::clone(search_mode),

            killers: [[None; 2]; Searcher::MAX_PLY],
            history_heuristic: Arc::clone(history_heuristic),
            age,
            tt: Arc::clone(tt),
        }
    }

    #[inline(always)]
    fn ctx(&self, ply: usize, hash_move: Option<Move>) -> SearchContext<'_> {
        SearchContext {
            board: &self.board,
            pv_line: &self.prev_pv,
            killers: &self.killers,
            history_heuristic: &self.history_heuristic,
            hash_move,
            ply,
        }
    }
}
