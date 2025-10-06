use rand::{Rng, SeedableRng};

use crate::{
    attacks::{
        movegen::{gen_sliding_attacks, get_occupancy},
        tables::{self, Magic, Offset},
    },
    board::Square,
};

mod attacks;
mod board;

fn find_magic(
    square: Square,
    relevant_mask: u64,
    directions: &[Offset],
) -> Result<(u64, usize, Vec<u64>), &'static str> {
    let bits = relevant_mask.count_ones() as usize;
    let len = 1 << bits;

    let occupancies: Vec<u64> = (0..len)
        .map(|variant| get_occupancy(variant, relevant_mask))
        .collect();
    let attacks: Vec<u64> = (0..len)
        .map(|variant| gen_sliding_attacks(square, occupancies[variant], directions))
        .collect();

    let mut rng = rand::rngs::SmallRng::seed_from_u64(1);

    for _ in 0..100_000_000 {
        let mut used: Vec<u64> = vec![0; len];
        let magic = rng.random::<u64>() & rng.random::<u64>() & rng.random::<u64>();

        let mut collided = false;
        for variant in 0..len {
            let occupancy = occupancies[variant];
            let magic_index: usize =
                ((occupancy.wrapping_mul(magic)) >> (board::BOARD_SIZE - bits)) as usize;

            if used[magic_index] == 0 {
                used[magic_index] = attacks[variant];
            } else if used[magic_index] != attacks[variant] {
                collided = true;
                break;
            }
        }

        if !collided {
            return Ok((magic, bits, used));
        }
    }

    Err("Didn't find magic")
}

pub fn main() -> Result<(), &'static str> {
    let mut bishop_magics: [Magic; board::BOARD_SIZE] = [Magic {
        offset: 0,
        magic: 0,
        shift: 0,
    }; board::BOARD_SIZE];
    let mut rook_magics: [Magic; board::BOARD_SIZE] = [Magic {
        offset: 0,
        magic: 0,
        shift: 0,
    }; board::BOARD_SIZE];
    let mut sliding_attacks: Vec<u64> = vec![];
    let mut offset = 0;

    print!("use crate::attacks::tables::Magic;\n\n");

    for square in 0..board::BOARD_SIZE {
        let (magic, bits, mut att_sq) = find_magic(
            square as Square,
            tables::BISHOP_RM[square],
            &tables::BISHOP_DIRECTIONS,
        )?;

        bishop_magics[square] = Magic {
            offset: offset,
            magic: magic,
            shift: board::BOARD_SIZE - bits,
        };
        sliding_attacks.append(&mut att_sq);

        offset += 1usize << bits;
    }
    println!(
        "pub const BISHOP_MAGICS: [Magic; 64] = {:?};",
        bishop_magics
    );

    for square in 0..board::BOARD_SIZE {
        let (magic, bits, mut att_sq) = find_magic(
            square as Square,
            tables::ROOK_RM[square],
            &tables::ROOK_DIRECTIONS,
        )?;

        rook_magics[square] = Magic {
            offset: offset,
            magic: magic,
            shift: board::BOARD_SIZE - bits,
        };
        sliding_attacks.append(&mut att_sq);

        offset += 1usize << bits;
    }
    println!("\npub const ROOK_MAGICS: [Magic; 64] = {:?};", rook_magics);

    println!(
        "\npub const SLIDING_ATTACKS: [u64; {}] = {:?};",
        sliding_attacks.len(),
        sliding_attacks
    );

    Ok(())
}
