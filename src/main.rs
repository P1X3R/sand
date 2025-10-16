mod chess;

use chess::{attacks, board};

fn _print_bitboard(bitboard: u64) {
    println!();
    for rank in (0..board::BOARD_WIDTH).rev() {
        print!("{}  ", rank + 1);
        for file in 0..board::BOARD_WIDTH {
            let sq = rank * board::BOARD_WIDTH + file;
            let bit = 1u64 << sq;
            print!("{} ", if bitboard & bit != 0 { '●' } else { '·' });
        }
        println!();
    }
    println!("\n   a b c d e f g h\n");
}

fn main() -> Result<(), &'static str> {
    let board = board::Board::new("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")?;

    print!("Pseudo-legal moves: ");
    for mov in attacks::movegen::gen_color_moves(&board) {
        print!("{}", mov.to_uci());
    }
    println!();

    Ok(())
}
