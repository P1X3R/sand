pub const BOARD_WIDTH: usize = 8;
pub const BOARD_SIZE: usize = 64;

pub type Square = u8;

#[derive(PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub fn toggle(self) -> Color {
        [Color::White, Color::Black][self as usize ^ 1]
    }
}

#[repr(u8)]
pub enum Piece {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
    None,
}

#[repr(transparent)]
pub struct Castling(u8);

impl Castling {
    pub fn white_king_side(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn white_queen_side(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn black_king_side(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn black_queen_side(self) -> bool {
        self.0 & 8 != 0
    }
}

pub struct Board {
    pieces: [(Piece, Color); BOARD_SIZE],
    bitboards: [[u64; 6]; 2], // 6 piece types for 2 colors
    occupancies: [u64; 2],

    zobrist: u64,
    en_passant_sq: Option<Square>,
    halfmove_clock: u8,
    castling_rights: Castling, // 4 bits for KQkq
    side_to_move: Color,
}

pub const RANKS: [u64; BOARD_WIDTH] = [
    0xFF,
    0xFF00,
    0xFF0000,
    0xFF000000,
    0xFF00000000,
    0xFF0000000000,
    0xFF000000000000,
    0xFF00000000000000,
];

#[inline]
pub fn to_square(rank: i8, file: i8) -> Square {
    ((rank * BOARD_WIDTH as i8) + file) as Square
}

#[inline]
pub fn valid_axis(axis: i8) -> bool {
    axis >= 0 && axis < BOARD_WIDTH as i8
}
