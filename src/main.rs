mod chess;

use crate::chess::attacks::movegen::is_legal_move;
use chess::{attacks, board};
use std::time::Instant;

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
    let board =
        board::Board::new("r1bqkb1r/pppp1ppp/2n2n2/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4")?;
    println!("Zobrist: {}", board.zobrist);

    let program_start = Instant::now();
    let move_list = attacks::movegen::gen_color_moves(&board);
    println!("Time for move generation: {:?}", program_start.elapsed());

    for mov in move_list {
        let mut temp = board.clone();
        temp.make_move(mov);

        let start = Instant::now();
        if is_legal_move(mov, &temp) {
            let time_spent = start.elapsed();
            println!("{}Time for legality check: {:?}", mov.to_uci(), time_spent);
        }
    }

    Ok(())
}
