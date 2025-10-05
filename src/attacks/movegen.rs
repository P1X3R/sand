use crate::{
    attacks::{
        magics,
        tables::{self, Offset},
    },
    board::*,
};

pub fn gen_pawn_pushes(sq: isize, occ: u64, color: Color) -> u64 {
    assert!(sq >= 0 && sq < BOARD_SIZE as isize);

    let pawn_bit: u64 = 1u64 << sq;

    match color {
        Color::White => {
            let single: u64 = (pawn_bit << BOARD_WIDTH) & !occ;
            let double: u64 = ((single & RANK_BB_3) << BOARD_WIDTH) & !occ;

            single | double
        }
        Color::Black => {
            let single: u64 = (pawn_bit >> BOARD_WIDTH) & !occ;
            let double: u64 = ((single & RANK_BB_6) >> BOARD_WIDTH) & !occ;

            single | double
        }
    }
}

pub fn gen_jumping_attacks(sq: isize, offsets: &[Offset]) -> u64 {
    assert!(sq >= 0 && sq < BOARD_SIZE as isize);

    let rank = sq / BOARD_WIDTH as isize;
    let file = sq % BOARD_WIDTH as isize;

    offsets.iter().fold(0u64, |attacks, offset| {
        let (r, f) = (rank + offset.rank, file + offset.file);
        if valid_axis(r) && valid_axis(f) {
            attacks | (1u64 << to_square(r, f))
        } else {
            attacks
        }
    })
}

pub fn gen_edge_mask(sq: usize) -> u64 {
    assert!(sq < BOARD_SIZE);

    let bit: u64 = 1u64 << sq;

    const FILE_BB_1: u64 = 0x0101010101010101;
    const FILE_BB_8: u64 = 0x8080808080808080;

    [RANK_BB_1, RANK_BB_8, FILE_BB_1, FILE_BB_8]
        .iter()
        .fold(
            0u64,
            |mask, edge| if bit & edge == 0 { mask | edge } else { mask },
        )
}

pub fn gen_sliding_attacks(sq: isize, occ: u64, directions: &[Offset]) -> u64 {
    assert!(sq >= 0 && sq < BOARD_SIZE as isize);

    let rank = sq / BOARD_WIDTH as isize;
    let file = sq % BOARD_WIDTH as isize;

    let mut attacks: u64 = 0;

    for offset in directions {
        let (mut r, mut f) = (rank + offset.rank, file + offset.file);
        let mut ray: u64 = 0;

        while valid_axis(r) && valid_axis(f) {
            ray |= 1u64 << to_square(r, f);

            if ray & occ != 0 {
                break;
            }

            r += offset.rank;
            f += offset.file;
        }

        attacks |= ray;
    }

    attacks
}

pub fn get_occupancy(mut variant: usize, mut relevant_mask: u64) -> u64 {
    assert!(variant < (1 << relevant_mask.count_ones()));

    let mut occupancy: u64 = 0;

    while variant != 0 {
        if variant & 1 != 0 {
            occupancy |= relevant_mask & relevant_mask.wrapping_neg();
        }

        variant >>= 1;
        relevant_mask &= relevant_mask - 1;
    }

    occupancy
}

pub fn get_sliding_index(sq: isize, occ: u64, for_bishop: bool) -> usize {
    assert!(sq >= 0 && sq < BOARD_SIZE as isize);

    let relevant_mask = if for_bishop {
        tables::BISHOP_RM[sq as usize]
    } else {
        tables::ROOK_RM[sq as usize]
    };
    let magic = if for_bishop {
        magics::BISHOP_MAGICS[sq as usize]
    } else {
        magics::ROOK_MAGICS[sq as usize]
    };
    let variant = (occ & relevant_mask).wrapping_mul(magic.magic) >> magic.shift;
    assert!(variant < (1 << relevant_mask.count_ones()));

    magic.offset + variant as usize
}
