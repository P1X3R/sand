mod chess;

use crate::chess::{attacks::movegen::is_legal_move, make_move::Undo};
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
    let mut board =
        board::Board::new("r1bqkb1r/pppp1ppp/2n2n2/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4")?;
    let original = board.clone();
    println!("Undo struct size: {} bytes", std::mem::size_of::<Undo>());
    println!(
        "Board struct size: {} bytes",
        std::mem::size_of::<board::Board>()
    );
    println!("Zobrist: {}", board.zobrist);

    let program_start = Instant::now();
    let move_list = attacks::movegen::gen_color_moves(&board);
    println!("Time for move generation: {:?}", program_start.elapsed());

    for mov in move_list {
        let move_making_start = Instant::now();
        let undo = board.make_move(mov);
        let move_making_time = move_making_start.elapsed();

        let legality_check_start = Instant::now();
        let is_legal = is_legal_move(mov, &board);
        let legality_check_time = legality_check_start.elapsed();

        let undo_start = Instant::now();
        board.undo_move(&undo);
        let undo_time = undo_start.elapsed();

        assert_eq!(board, original);

        if is_legal {
            println!(
                "{:}Time for move making: {:?}; Time for legality check: {:?}; Time for undo: {:?}",
                mov.to_uci(),
                move_making_time,
                legality_check_time,
                undo_time
            );
        }
    }

    Ok(())
}
