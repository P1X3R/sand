use crate::chess::*;
use std::sync::LazyLock;

#[derive(Copy, Clone)]
pub struct Offset {
    pub rank: i8,
    pub file: i8,
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

pub static KNIGHT_ATTACKS: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|square| gen_jumping_attacks(square as Square, &KNIGHT_OFFSETS))
});
pub static KING_ATTACKS: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|square| gen_jumping_attacks(square as Square, &KING_OFFSETS))
});
pub static WPAWN_ATTACKS: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|square| gen_jumping_attacks(square as Square, &PAWN_CAPTURE_OFFSETS_WHITE))
});
pub static BPAWN_ATTACKS: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|square| gen_jumping_attacks(square as Square, &PAWN_CAPTURE_OFFSETS_BLACK))
});
pub static BISHOP_RM: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|square| {
        gen_sliding_attacks(square as Square, 0, &BISHOP_DIRECTIONS)
            & !gen_edge_mask(square as Square)
    })
});
pub static ROOK_RM: LazyLock<[u64; BOARD_SIZE]> = LazyLock::new(|| {
    std::array::from_fn(|square| {
        gen_sliding_attacks(square as Square, 0, &ROOK_DIRECTIONS)
            & !gen_edge_mask(square as Square)
    })
});

// Flags are encoded like this:
// 1 bit    |1 bit  |1 bit   |1 bit
// Promotion|Capture|Special1|Special0
pub const FLAGS_LUT: [MoveFlag; 16] = [
    // 0000
    MoveFlag {
        move_type: MoveType::Quiet,
        promotion: Piece::None,
    },
    // 0001
    MoveFlag {
        move_type: MoveType::DoublePawnPush,
        promotion: Piece::None,
    },
    // 0010
    MoveFlag {
        move_type: MoveType::KingSideCastle,
        promotion: Piece::None,
    },
    // 0011
    MoveFlag {
        move_type: MoveType::QueenSideCastle,
        promotion: Piece::None,
    },
    // 0100
    MoveFlag {
        move_type: MoveType::Capture,
        promotion: Piece::None,
    },
    // 0101
    MoveFlag {
        move_type: MoveType::EnPassantCapture,
        promotion: Piece::None,
    },
    // 0110
    MoveFlag {
        move_type: MoveType::Invalid,
        promotion: Piece::None,
    },
    // 0111
    MoveFlag {
        move_type: MoveType::Invalid,
        promotion: Piece::None,
    },
    // 1000
    MoveFlag {
        move_type: MoveType::Quiet,
        promotion: Piece::Knight,
    },
    // 1001
    MoveFlag {
        move_type: MoveType::Quiet,
        promotion: Piece::Bishop,
    },
    // 1010
    MoveFlag {
        move_type: MoveType::Quiet,
        promotion: Piece::Rook,
    },
    // 1011
    MoveFlag {
        move_type: MoveType::Quiet,
        promotion: Piece::Queen,
    },
    // 1100
    MoveFlag {
        move_type: MoveType::Capture,
        promotion: Piece::Knight,
    },
    // 1101
    MoveFlag {
        move_type: MoveType::Capture,
        promotion: Piece::Bishop,
    },
    // 1110
    MoveFlag {
        move_type: MoveType::Capture,
        promotion: Piece::Rook,
    },
    // 1111
    MoveFlag {
        move_type: MoveType::Capture,
        promotion: Piece::Queen,
    },
];
