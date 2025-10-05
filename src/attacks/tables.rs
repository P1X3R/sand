use crate::{
    attacks::movegen::{gen_edge_mask, gen_jumping_attacks, gen_sliding_attacks},
    board::BOARD_SIZE,
};
use std::sync::LazyLock;

#[derive(Copy, Clone)]
pub struct Offset {
    pub rank: isize,
    pub file: isize,
}

#[derive(Clone, Copy, Debug)]
pub struct Magic {
    pub offset: usize,
    pub magic: u64,
    pub shift: usize,
}

pub const PAWN_CAPTURE_OFFSETS_WHITE: [Offset; 2] = [
    Offset { rank: 1, file: -1 }, // capture left
    Offset { rank: 1, file: 1 },  // capture right
];

pub const PAWN_CAPTURE_OFFSETS_BLACK: [Offset; 2] = [
    Offset { rank: -1, file: -1 }, // capture left
    Offset { rank: -1, file: 1 },  // capture right
];

pub const KNIGHT_OFFSETS: [Offset; 8] = [
    Offset { rank: 2, file: 1 },
    Offset { rank: 1, file: 2 },
    Offset { rank: -1, file: 2 },
    Offset { rank: -2, file: 1 },
    Offset { rank: -2, file: -1 },
    Offset { rank: -1, file: -2 },
    Offset { rank: 1, file: -2 },
    Offset { rank: 2, file: -1 },
];

pub const KING_OFFSETS: [Offset; 8] = [
    Offset { rank: 1, file: 0 },
    Offset { rank: 1, file: 1 },
    Offset { rank: 0, file: 1 },
    Offset { rank: -1, file: 1 },
    Offset { rank: -1, file: 0 },
    Offset { rank: -1, file: -1 },
    Offset { rank: 0, file: -1 },
    Offset { rank: 1, file: -1 },
];

pub const ROOK_DIRECTIONS: [Offset; 4] = [
    Offset { rank: 1, file: 0 },  // north
    Offset { rank: -1, file: 0 }, // south
    Offset { rank: 0, file: 1 },  // east
    Offset { rank: 0, file: -1 }, // west
];

pub const BISHOP_DIRECTIONS: [Offset; 4] = [
    Offset { rank: 1, file: 1 },   // northeast
    Offset { rank: 1, file: -1 },  // northwest
    Offset { rank: -1, file: 1 },  // southeast
    Offset { rank: -1, file: -1 }, // southwest
];

pub static KNIGHT_ATTACKS: LazyLock<[u64; BOARD_SIZE]> =
    LazyLock::new(|| std::array::from_fn(|sq| gen_jumping_attacks(sq as isize, &KNIGHT_OFFSETS)));
pub static KING_ATTACKS: LazyLock<[u64; BOARD_SIZE]> =
    LazyLock::new(|| std::array::from_fn(|sq| gen_jumping_attacks(sq as isize, &KING_OFFSETS)));
pub static WPAWN_ATTACKS: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|sq| gen_jumping_attacks(sq as isize, &PAWN_CAPTURE_OFFSETS_WHITE))
});
pub static BPAWN_ATTACKS: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|sq| gen_jumping_attacks(sq as isize, &PAWN_CAPTURE_OFFSETS_BLACK))
});
pub static BISHOP_RM: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|sq| {
        gen_sliding_attacks(sq as isize, 0, &BISHOP_DIRECTIONS) & !gen_edge_mask(sq)
    })
});
pub static ROOK_RM: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|sq| {
        gen_sliding_attacks(sq as isize, 0, &ROOK_DIRECTIONS) & !gen_edge_mask(sq)
    })
});
