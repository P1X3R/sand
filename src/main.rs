mod chess;

use crate::chess::{attacks::movegen::is_legal_move, moves::MoveType};
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
    let color = board.side_to_move;
    println!("Zobrist: {}", board.zobrist);

    print!("Pseudo-legal moves: ");

    let start = Instant::now();
    for mov in attacks::movegen::gen_color_moves(&board) {
        let from = mov.get_from();
        let to = mov.get_to();
        let flags = mov.get_flags();
        let to_square_info = board.pieces[to as usize];
        let piece_type = board.pieces[from as usize].0;

        board.toggle_piece(from, piece_type, color);
        if flags.move_type == MoveType::Capture {
            board.toggle_piece(to, to_square_info.0, to_square_info.1);
        }
        board.toggle_piece(to, piece_type, color);

        if is_legal_move(mov, &board) {
            print!("{}", mov.to_uci());
        }

        board.toggle_piece(from, piece_type, color);
        board.toggle_piece(to, piece_type, color);
        if flags.move_type == MoveType::Capture {
            board.toggle_piece(to, to_square_info.0, to_square_info.1);
        }
    }
    println!();
    println!("Elapsed time: {:?}", start.elapsed());

    Ok(())
}
