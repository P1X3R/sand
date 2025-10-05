use crate::{
    attacks::{
        magics,
        movegen::{gen_jumping_attacks, get_sliding_index},
    },
    board::BOARD_SIZE,
};

mod attacks;
mod board;

fn print_bitboard(bb: u64) {
    println!();
    for rank in (0..board::BOARD_WIDTH).rev() {
        print!("{}  ", rank + 1);
        for file in 0..board::BOARD_WIDTH {
            let sq = rank * board::BOARD_WIDTH + file;
            let bit = 1u64 << sq;
            print!("{} ", if bb & bit != 0 { '●' } else { '·' });
        }
        println!();
    }
    println!("\n   a b c d e f g h\n");
}

fn main() {
    let occ = 1u64 << 9;
    print_bitboard(occ);
    print_bitboard(
        attacks::magics::SLIDING_ATTACKS[attacks::movegen::get_sliding_index(0, occ, true)],
    );
}
