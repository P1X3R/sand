use crate::{
    chess::{
        attacks::movegen::{gen_color_moves, is_legal_move},
        board::{Board, STARTPOS_FEN},
        moves::Move,
    },
    search::Engine,
};
use std::str::SplitWhitespace;
use tinyvec::ArrayVec;

pub struct UCI {
    // canonical position & history used when parsing `position`
    position_board: Board,
    position_history: ArrayVec<[u64; 1024]>,

    engine: Engine,
}

impl UCI {
    /// Return if is `quit` command
    fn execute_commands(&mut self, tokens: &mut SplitWhitespace) -> bool {
        match tokens.next() {
            Some("uci") => {
                println!("id name Sand");
                println!("id author P1x3r");
                println!("uciok");
            }
            Some("debug") => {}
            Some("isready") => println!("readyok"),
            Some("setoption") => todo!(),
            Some("register") => println!("registration ok"),
            Some("ucinewgame") => self.engine = Engine::new(),
            Some("position") => {
                if let Err(e) = self.handle_position(tokens) {
                    println!("info string position error {e}");
                }
            }
            Some("go") => todo!(),
            Some("stop") => self.engine.stop_search(),
            Some("ponderhit") => todo!(),
            Some("quit") => return true,
            _ => println!("info string unknown command"),
        };

        false
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

        tokens.next(); // Skip `moves`

        while let Some(move_uci) = tokens.next() {
            let mov = Move::from_uci(move_uci, &self.position_board)?;
            let move_list = gen_color_moves(&self.position_board);

            if move_list.contains(&mov) && is_legal_move(mov, &self.position_board) {
                self.position_board.make_move(mov);
                self.position_history.push(self.position_board.zobrist);
            } else {
                break; // One illegal move, makes the rest illegal
            }
        }

        Ok(())
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

    pub fn new() -> UCI {
        UCI {
            engine: Engine::new(),
            position_board: Board::new(STARTPOS_FEN).unwrap(),
            position_history: ArrayVec::new(),
        }
    }
}
