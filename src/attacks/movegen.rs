use crate::{
    attacks::{
        magics,
        tables::{self, Offset},
    },
    board::*,
};

pub fn gen_pawn_pushes(square: Square, occupancy: u64, color: Color) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    let pawn_bit: u64 = 1u64 << square;

    match color {
        Color::White => {
            let single: u64 = (pawn_bit << BOARD_WIDTH) & !occupancy;
            let double: u64 = ((single & RANKS[2]) << BOARD_WIDTH) & !occupancy;

            single | double
        }
        Color::Black => {
            let single: u64 = (pawn_bit >> BOARD_WIDTH) & !occupancy;
            let double: u64 = ((single & RANKS[5]) >> BOARD_WIDTH) & !occupancy;

            single | double
        }
    }
}

pub fn gen_jumping_attacks(square: Square, offsets: &[Offset]) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    let rank = square as i8 / BOARD_WIDTH as i8;
    let file = square as i8 % BOARD_WIDTH as i8;

    offsets.iter().fold(0u64, |attacks, offset| {
        let (r, f) = (rank + offset.rank, file + offset.file);
        if valid_axis(r) && valid_axis(f) {
            attacks | (1u64 << to_square(r, f))
        } else {
            attacks
        }
    })
}

pub fn gen_edge_mask(square: usize) -> u64 {
    debug_assert!(square < BOARD_SIZE);

    let bit: u64 = 1u64 << square;

    const FILE_BB_1: u64 = 0x0101010101010101;
    const FILE_BB_8: u64 = 0x8080808080808080;

    [RANKS[0], RANKS[7], FILE_BB_1, FILE_BB_8]
        .iter()
        .fold(
            0u64,
            |mask, edge| if bit & edge == 0 { mask | edge } else { mask },
        )
}

pub fn gen_sliding_attacks(square: Square, occupancy: u64, directions: &[Offset]) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    let rank = square as i8 / BOARD_WIDTH as i8;
    let file = square as i8 % BOARD_WIDTH as i8;

    let mut attacks: u64 = 0;

    for offset in directions {
        let (mut r, mut f) = (rank + offset.rank, file + offset.file);
        let mut ray: u64 = 0;

        while valid_axis(r) && valid_axis(f) {
            ray |= 1u64 << to_square(r, f);

            if ray & occupancy != 0 {
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
    debug_assert!(variant < (1 << relevant_mask.count_ones()));

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

#[inline(always)]
pub fn get_bishop_index(square: usize, occupancy: u64) -> usize {
    let magic = &magics::BISHOP_MAGICS[square];
    let variant = (occupancy & tables::BISHOP_RM[square]).wrapping_mul(magic.magic) >> magic.shift;
    debug_assert!(variant < (1 << tables::BISHOP_RM[square].count_ones()));
    magic.offset + variant as usize
}

#[inline(always)]
pub fn get_rook_index(square: usize, occupancy: u64) -> usize {
    let magic = &magics::ROOK_MAGICS[square];
    let variant = (occupancy & tables::ROOK_RM[square]).wrapping_mul(magic.magic) >> magic.shift;
    debug_assert!(variant < (1 << tables::ROOK_RM[square].count_ones()));
    magic.offset + variant as usize
}
