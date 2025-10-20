mod chess;
mod evaluation;
mod search;
mod uci;

pub fn main() {
    let mut uci = uci::UCI::new();
    uci.uci_loop();
}
