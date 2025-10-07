mod chess;

use chess::{attacks, board};

fn print_bitboard(bitboard: u64) {
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

fn main() {
    let occupancy = 1u64 << 9;
    print_bitboard(occupancy);
    print_bitboard(
        attacks::magics::SLIDING_ATTACKS[attacks::movegen::get_rook_index(0, occupancy)],
    );
}
