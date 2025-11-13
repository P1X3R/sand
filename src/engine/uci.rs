use crate::{chess::*, engine::search::*};
use std::{str::SplitWhitespace, sync::Arc, thread::JoinHandle};
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

    worker: Option<JoinHandle<()>>,
    search_mode: Arc<AtomicSearchMode>,
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
    /// return true if is `quit` command
    fn execute_commands(&mut self, tokens: &mut SplitWhitespace) -> bool {
        match tokens.next() {
            Some("uci") => {
                send!("id name Sand");
                send!("id author P1x3r");
                send!("option name Ponder type check default false");
                send!("uciok");
            }
            Some("debug") => {}
            Some("isready") => send!("readyok"),
            Some("setoption") => {}
            Some("register") => send!("registration ok"),
            Some("ucinewgame") => {
                self.stop_and_join();

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
            Some("stop") => self.search_mode.store(SearchMode::Stop),
            Some("ponderhit") => {
                if self.search_mode.load() == SearchMode::Ponder {
                    self.search_mode.store(SearchMode::PonderHit);
                }
            }
            Some("quit") => {
                self.stop_and_join();
                return true;
            }
            Some("eval") => {
                send!("bonus: {:?}", self.position_board.bonus);
                send!("material: {:?}", self.position_board.material);
                send!("phase: {}", self.position_board.phase);
                send!(
                    "static eval: {}",
                    match self.position_board.side_to_move {
                        Color::White => self.position_board.evaluate(),
                        Color::Black => -self.position_board.evaluate(),
                    }
                );
            }
            None => {}
            _ => send!("info string unknown command"),
        };

        false
    }

    fn stop_and_join(&mut self) {
        self.search_mode.store(SearchMode::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    fn handle_position(&mut self, tokens: &mut SplitWhitespace) -> Result<(), &'static str> {
        let fen: String = match tokens.next() {
            Some("startpos") => STARTPOS_FEN.to_string(),
            Some("fen") => tokens
                .by_ref()
                .take_while(|&t| t != "moves")
                .collect::<Vec<&str>>()
                .join(" "),
            _ => STARTPOS_FEN.to_string(),
        };

        self.position_board = Board::new(&fen)?;
        self.position_history.clear();
        self.position_history.push(self.position_board.zobrist);

        if tokens.next() == Some("moves") {
            for move_uci in tokens {
                // check pseudo-legality
                let move_list = gen_color_moves(&self.position_board);
                let Some(&mov) = move_list.iter().find(|m| m.to_uci() == move_uci) else {
                    continue; // Silently ignore invalid moves
                };

                self.position_board.make_move(mov);
                self.position_history.push(self.position_board.zobrist);
            }
        }

        Ok(())
    }

    fn handle_go(&mut self, tokens: &mut SplitWhitespace) {
        let mut clock_time = ClockTime::default();
        let mut has_clock_time = false;
        let mut time_control = TimeControl::Infinite;

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
                        "movetime" => time_control = TimeControl::MoveTime(val),
                        "depth" => time_control = TimeControl::Depth(val as usize),
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
                "infinite" => time_control = TimeControl::Infinite,
                "ponder" => self.search_mode.store(SearchMode::Ponder),
                _ => {}
            }
        }

        if has_clock_time {
            time_control = TimeControl::ClockTime(clock_time);
        }

        let mut searcher = Searcher::new(
            self.position_board.clone(),
            self.position_history,
            &self.search_mode,
        );

        self.worker = Some(std::thread::spawn(move || {
            let (best_move, ponder_move) = searcher.start_search(time_control);
            if let Some(p) = ponder_move {
                send!("bestmove {} ponder {}", best_move.to_uci(), p.to_uci());
            } else {
                send!("bestmove {}", best_move.to_uci());
            }
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
            position_board: Board::new(STARTPOS_FEN).unwrap(),
            position_history: ArrayVec::new(),

            worker: None,
            search_mode: Arc::new(AtomicSearchMode::new(SearchMode::Normal)),
        }
    }
}
