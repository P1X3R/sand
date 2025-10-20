use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use crate::chess::{
    attacks::movegen::{gen_color_moves, is_legal_move, is_square_attacked},
    board::{Board, Color, Piece, Square},
    make_move::Undo,
    moves::Move,
};
use tinyvec::{Array, ArrayVec};

#[derive(Clone, Copy)]
pub(self) struct ClockTime {
    white_time_ms: u64,
    black_time_ms: u64,
    white_increment_ms: u64,
    black_increment_ms: u64,
}

pub struct TimeControl {
    move_time: Option<u64>,
    depth: Option<usize>,
    clock_time: Option<ClockTime>,
    infinite: bool,
    ponder: bool,
}

pub struct Engine {
    board: Board,
    history: ArrayVec<[u64; 1024]>, // For three-fold repetition, (zobrist, is irreversible)
    time_control: TimeControl,
    stop: Arc<AtomicBool>,

    nodes: usize,
}

impl Engine {
    const MAX_PLY: usize = 64;
    const CHECKMATE_SCORE: i16 = 20_000;
    const CHECKMATE_THRESHOLD: i16 = Engine::CHECKMATE_SCORE - Engine::MAX_PLY as i16;
    const INF: i16 = 30_000;
    const DRAW_SCORE: i16 = 0;

    fn is_draw(&self) -> bool {
        self.board.is_fifty_move()
            || self
                .history
                .iter()
                .take(self.board.halfmove_clock as usize)
                .rev()
                .filter(|&zobrist| *zobrist == self.board.zobrist)
                .take(3)
                .count()
                == 3
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
        if self.stop.load(Ordering::Relaxed) {
            self.stop.store(false, Ordering::Relaxed);
        }

        if control.ponder {
            // Set to 0 ms by default to automatically stop once `ponderhit` if GUI sent just:
            // `go ponder` with no time control
            let time_ms = control.clock_time.as_ref().map_or(0, |clock_time| {
                Engine::calculate_time_from_clock(self.board.side_to_move, clock_time)
            });

            return self.iterative_deepening(Some(time_ms), None);
        } else if let Some(move_time) = control.move_time {
            return self.iterative_deepening(Some(move_time), None);
        } else if let Some(depth) = control.depth {
            return self.iterative_deepening(None, Some(depth));
        } else if let Some(clock_time) = &control.clock_time {
            return self.iterative_deepening(
                Some(Engine::calculate_time_from_clock(
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

    fn time_to_stop(&self, start: &Instant, time_ms: Option<u64>) -> bool {
        self.stop.load(Ordering::Relaxed)
            || time_ms.is_some_and(|ms| {
                !self.time_control.ponder && start.elapsed().as_millis() >= ms as u128
            })
    }

    fn iterative_deepening(&mut self, time_ms: Option<u64>, depth: Option<usize>) -> Move {
        let move_list = gen_color_moves(&self.board);
        let mut best_move: Move = move_list[0];
        let mut current_depth = 1;
        let start = Instant::now();

        loop {
            let mut step_best_move = best_move;
            let mut best_score = -Engine::INF;
            for mov in move_list {
                if self.time_to_stop(&start, time_ms) {
                    break;
                }

                let undo = self.push_move(mov);
                if is_legal_move(mov, &self.board) {
                    let score = self.search(
                        -Engine::INF,
                        Engine::INF,
                        current_depth - 1,
                        1,
                        &start,
                        time_ms,
                    );
                    if score > best_score {
                        step_best_move = mov;
                        best_score = score;
                    }
                }
                self.pop_move(&undo);
            }

            if self.time_to_stop(&start, time_ms)
                || current_depth >= depth.unwrap_or(Engine::MAX_PLY)
            {
                break;
            }

            best_move = step_best_move;
            current_depth += 1;
            self.nodes = 0;
        }

        // Prepare for next search
        if self.stop.load(Ordering::Relaxed) {
            self.stop.store(false, Ordering::Relaxed);
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
        start: &Instant,
        time_ms: Option<u64>,
    ) -> i16 {
        self.nodes += 1;
        let color = self.board.side_to_move;

        if self.is_draw() {
            return Engine::DRAW_SCORE;
        }
        if depth == 0 || (self.nodes & 1023 == 0 && self.time_to_stop(start, time_ms)) {
            return match color {
                Color::White => self.board.evaluate(),
                Color::Black => -self.board.evaluate(),
            };
        }

        let mate_score = Engine::CHECKMATE_SCORE - ply as i16;
        let mut best_score = -Engine::INF;
        let mut found_legal_move = false;

        for mov in gen_color_moves(&self.board) {
            let undo = self.push_move(mov);
            if !is_legal_move(mov, &self.board) {
                self.pop_move(&undo);
                continue;
            }

            found_legal_move = true;
            let score = -self.search(-beta, -alpha, depth - 1, ply + 1, start, time_ms);

            self.pop_move(&undo);

            if score > best_score {
                best_score = score;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha > beta {
                return alpha;
            }
            if self.nodes & 1023 == 0 && self.time_to_stop(start, time_ms) {
                return alpha;
            }
        }

        if !found_legal_move {
            let in_check = is_square_attacked(
                self.board.bitboards[color as usize][Piece::King as usize].trailing_zeros()
                    as Square,
                color.toggle(),
                &self.board,
            );

            return if in_check {
                -mate_score
            } else {
                Engine::DRAW_SCORE
            };
        }

        best_score
    }

    pub fn new() -> Engine {
        Engine {
            board: Board::new("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 ").unwrap(),
            history: ArrayVec::new(),
            time_control: TimeControl {
                move_time: None,
                depth: None,
                clock_time: None,
                infinite: false,
                ponder: false,
            },
            stop: Arc::from(AtomicBool::new(false)),
            nodes: 0,
        }
    }

    pub fn stop_search(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
