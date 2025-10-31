use crate::{
    chess::{
        attacks::movegen::{gen_color_moves, is_legal_move},
        board::{Board, STARTPOS_FEN},
    },
    search::{ClockTime, Searcher, TimeControl},
};
use std::{str::SplitWhitespace, thread::JoinHandle};
use tinyvec::ArrayVec;

#[macro_export]
macro_rules! send {
    ($($arg:tt)*) => {{
        use std::io::{self, Write};
        println!($($arg)*);
        io::stdout().flush().unwrap();
    }};
}

pub struct Uci {
    // canonical position & history used when parsing `position`
    position_board: Board,
    position_history: ArrayVec<[u64; 1024]>,

    searcher: Searcher,
    worker: Option<JoinHandle<()>>,
}

fn perft(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1u64;
    }

    let mut nodes = 0u64;

    for mov in gen_color_moves(board) {
        let undo = board.make_move(mov);
        if is_legal_move(mov, board) {
            debug_assert_eq!(board.zobrist, board.calculate_zobrist());
            nodes += perft(board, depth - 1);
        }
        board.undo_move(&undo);
    }

    nodes
}

fn divide(board: &mut Board, depth: usize) -> u64 {
    if depth == 0 {
        return 1u64;
    }

    let mut nodes = 0u64;

    for mov in gen_color_moves(board) {
        let undo = board.make_move(mov);
        if is_legal_move(mov, board) {
            debug_assert_eq!(board.zobrist, board.calculate_zobrist());
            let subtree_nodes = perft(board, depth - 1);
            nodes += subtree_nodes;
            send!("{}: {}", mov.to_uci(), subtree_nodes);
        }
        board.undo_move(&undo);
    }

    nodes
}

impl Uci {
    /// Return if is `quit` command
    fn execute_commands(&mut self, tokens: &mut SplitWhitespace) -> bool {
        match tokens.next() {
            Some("uci") => {
                send!("id name Sand");
                send!("id author P1x3r");
                send!("uciok");
            }
            Some("debug") => {}
            Some("isready") => send!("readyok"),
            Some("setoption") => todo!(),
            Some("register") => send!("registration ok"),
            Some("ucinewgame") => {
                self.stop_and_join();

                self.searcher = Searcher::new();
                self.position_board = Board::new(STARTPOS_FEN).unwrap();
                self.position_history = ArrayVec::new();
                self.worker = None;
            }
            Some("position") => {
                if let Err(e) = self.handle_position(tokens) {
                    send!("info string position error {e}");
                }
            }
            Some("go") => self.handle_go(tokens),
            Some("stop") => self.searcher.stop_search(),
            Some("ponderhit") => todo!(),
            Some("quit") => {
                self.stop_and_join();
                return true;
            }
            None => {}
            _ => send!("info string unknown command"),
        };

        false
    }

    fn stop_and_join(&mut self) {
        self.searcher.stop_search();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    fn handle_position(&mut self, tokens: &mut SplitWhitespace) -> Result<(), &'static str> {
        let fen: String = match tokens.next() {
            Some("fen") => {
                let mut f = String::new();
                for part in tokens.take_while(|&t| t != "moves") {
                    if !f.is_empty() {
                        f.push(' ');
                    }
                    f.push_str(part);
                }
                f
            }
            _ => STARTPOS_FEN.to_string(), // Handles `startpos` indirectly
        };

        self.position_board = Board::new(&fen)?;
        self.position_history.clear();

        if tokens.next() == Some("moves") {
            for move_uci in tokens {
                // check pseudo-legality
                let move_list = gen_color_moves(&self.position_board);
                let Some(&mov) = move_list.iter().find(|m| m.to_uci() == move_uci) else {
                    continue;
                };

                self.position_board.make_move(mov);
                self.position_history.push(self.position_board.zobrist);
            }
        }

        self.searcher
            .copy_pos(&self.position_board, &self.position_history);
        Ok(())
    }

    fn handle_go(&mut self, tokens: &mut SplitWhitespace) {
        let mut clock_time = ClockTime::default();
        let mut has_clock_time = false;
        let mut time_control = TimeControl::default();

        while let Some(key) = tokens.next() {
            match key {
                "movetime" | "depth" | "wtime" | "btime" | "winc" | "binc" | "perft" => {
                    let Some(val) = tokens.next() else {
                        continue;
                    };
                    let Ok(val) = val.parse::<u64>() else {
                        continue;
                    };

                    match key {
                        "movetime" => time_control.move_time = Some(val),
                        "depth" => time_control.depth = Some(val as usize),
                        "wtime" => {
                            has_clock_time = true;
                            clock_time.white_time_ms = val;
                        }
                        "btime" => {
                            has_clock_time = true;
                            clock_time.black_time_ms = val;
                        }
                        "winc" => clock_time.white_increment_ms = val,
                        "binc" => clock_time.black_increment_ms = val,
                        "perft" => {
                            send!(
                                "Nodes searched: {}",
                                divide(&mut self.position_board, val as usize)
                            );
                            return; // intentional, perft must not search
                        }
                        _ => unreachable!(),
                    }
                }
                "infinite" => time_control.infinite = true,
                "ponder" => time_control.ponder = true,
                _ => {}
            }
        }

        time_control.clock_time = has_clock_time.then_some(clock_time);
        self.searcher.stop_search();

        let mut searcher = self.searcher.clone();
        searcher.time_control = time_control;
        self.worker = Some(std::thread::spawn(move || {
            send!("bestmove {}", searcher.start_search().to_uci());
        }));
    }

    pub fn uci_loop(&mut self) {
        let stdin = std::io::stdin();
        let mut input = String::new();

        loop {
            input.clear();
            if stdin.read_line(&mut input).is_err() {
                break;
            }
            if self.execute_commands(&mut input.split_whitespace()) {
                break;
            }
        }
    }

    pub fn new() -> Uci {
        Uci {
            searcher: Searcher::new(),
            position_board: Board::new(STARTPOS_FEN).unwrap(),
            position_history: ArrayVec::new(),
            worker: None,
        }
    }
}
