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

#[derive(Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MoveType {
    Quiet,
    DoublePawnPush,
    KingSideCastle,
    QueenSideCastle,
    Capture,
    EnPassantCapture,
    Invalid,
}

#[derive(Clone, Copy)]
pub struct MoveFlag {
    pub move_type: MoveType,
    pub promotion: Piece,
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(transparent)]
pub struct Move(pub u16);

impl Move {
    #[inline(always)]
    pub fn new(from: Square, to: Square, move_flags: MoveFlag) -> Self {
        debug_assert!(move_flags.move_type != MoveType::Invalid);
        debug_assert!(from < BOARD_SIZE as u8 && to < BOARD_SIZE as u8);

        // --- bit-field layout of the final u16 ---
        // 15..12 : 4-bit flags  (move type + promotion)
        // 11..6  : 6-bit destination square
        // 5..0   : 6-bit origin square

        // 1. destination square → bits 11..6
        let to_encoded = (to as u16) << 6;

        // 2. promotion piece → upper half of the 4-bit flags
        //    * if no promotion → 0000
        //    * else            → 1ppp   (ppp = promotion-1 so Knight=0, Bishop=1, Rook=2, Queen=3)
        let promotion_bits = if move_flags.promotion != Piece::None {
            0b1000 | ((move_flags.promotion as u16) - 1) // 0b1000 .. 0b1011
        } else {
            0
        };

        // 3. move type → lower half of the 4-bit flags (bits 3..0 before the final shift)
        //    promotion_bits occupy bits 3..0 as well, but never clash because
        //    promotion_bits has bit-3 always set (0b1xxx) while pure move types
        //    never exceed 0b0111 (EnPassantCapture = 5).
        let move_flags_encoded = ((move_flags.move_type as u16) | promotion_bits) << 12;

        // 4. pack everything together
        Move(from as u16 | to_encoded | move_flags_encoded)
    }

    #[inline(always)]
    pub fn from_square(self) -> Square {
        (self.0 & 0x3f) as Square
    }

    #[inline(always)]
    pub fn to_square(self) -> Square {
        (self.0 >> 6 & 0x3f) as Square
    }

    #[inline(always)]
    pub fn get_flags(self) -> MoveFlag {
        let encoded_flags = (self.0 >> 12 & 0xf) as usize;
        let move_flag = crate::attacks::tables::FLAGS_LUT[encoded_flags];

        debug_assert!(move_flag.move_type != MoveType::Invalid);
        move_flag
    }
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

#[inline(always)]
pub fn to_square(rank: i8, file: i8) -> Square {
    ((rank * BOARD_WIDTH as i8) + file) as Square
}

#[inline(always)]
pub fn valid_axis(axis: i8) -> bool {
    axis >= 0 && axis < BOARD_WIDTH as i8
}
