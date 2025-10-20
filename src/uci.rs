use crate::search::Engine;
use std::str::SplitWhitespace;

pub struct UCI {
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
            Some("position") => todo!(),
            Some("go") => todo!(),
            Some("stop") => self.engine.stop_search(),
            Some("ponderhit") => todo!(),
            Some("quit") => return true,
            _ => println!("info string unknown command"),
        };

        false
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
        }
    }
}
