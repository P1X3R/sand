pub const BOARD_WIDTH: usize = 8;
pub const BOARD_SIZE: usize = 64;

pub type Square = u8;

#[derive(PartialEq, Clone, Copy, Debug)]
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

impl Piece {
    pub fn from_char(letter: char) -> Result<Piece, &'static str> {
        match letter.to_ascii_lowercase() {
            'p' => Ok(Piece::Pawn),
            'n' => Ok(Piece::Knight),
            'b' => Ok(Piece::Bishop),
            'r' => Ok(Piece::Rook),
            'q' => Ok(Piece::Queen),
            'k' => Ok(Piece::King),
            _ => Err("Invalid character"),
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Piece::Pawn => 'P',
            Piece::Knight => 'N',
            Piece::Bishop => 'B',
            Piece::Rook => 'R',
            Piece::Queen => 'Q',
            Piece::King => 'K',
            Piece::None => ' ',
        }
    }
}

pub const PIECE_TYPES: [Piece; 6] = [
    Piece::Pawn,
    Piece::Knight,
    Piece::Bishop,
    Piece::Rook,
    Piece::Queen,
    Piece::King,
];

#[repr(transparent)]
pub struct Castling(u8);

impl Castling {
    pub fn white_king_side(&self) -> bool {
        self.0 & 1 != 0
    }
    pub fn white_queen_side(&self) -> bool {
        self.0 & 2 != 0
    }
    pub fn black_king_side(&self) -> bool {
        self.0 & 4 != 0
    }
    pub fn black_queen_side(&self) -> bool {
        self.0 & 8 != 0
    }
}

pub struct Board {
    pub pieces: [(Piece, Color); BOARD_SIZE],
    pub bitboards: [[u64; 6]; 2], // 6 piece types for 2 colors
    pub occupancies: [u64; 2],

    zobrist: u64,
    pub en_passant_square: Option<Square>,
    halfmove_clock: u8,
    castling_rights: Castling, // 4 bits for KQkq
    pub side_to_move: Color,
}

impl Board {
    pub fn toggle_piece(&mut self, square: Square, piece_type: Piece, color: Color) {
        let square_bit = bit(square);

        if self.pieces[square as usize].0 == Piece::None {
            self.pieces[square as usize] = (piece_type, color);
        } else {
            self.pieces[square as usize] = (Piece::None, Color::White);
        }
        self.bitboards[color as usize][piece_type as usize] ^= square_bit;
        self.occupancies[color as usize] ^= square_bit;

        // TODO: Zobrist
    }

    fn parse_positioning(&mut self, part: &str) -> Result<(), &'static str> {
        let mut rank: u8 = BOARD_WIDTH as u8 - 1;
        let mut file: u8 = 0;

        for chr in part.chars() {
            match chr {
                '/' => {
                    if rank == 0 {
                        return Err("Too many ranks");
                    }
                    rank -= 1;
                    file = 0;
                }
                c if c.is_ascii_digit() => {
                    file += c.to_digit(10).ok_or("Invalid digit")? as u8;
                }
                c => {
                    let piece_type = Piece::from_char(c)?;
                    let color = if c.is_uppercase() {
                        Color::White
                    } else {
                        Color::Black
                    };
                    if file >= BOARD_WIDTH as u8 {
                        return Err("File out of bounds");
                    }
                    self.toggle_piece(to_square(rank as i8, file as i8), piece_type, color);
                    file += 1;
                }
            }
        }

        if rank != 0 || file != BOARD_WIDTH as u8 {
            return Err("Incomplete board");
        }

        Ok(())
    }

    pub fn new(fen: String) -> Result<Self, &'static str> {
        let mut tokens = fen.trim().split(' ');

        let mut board = Board {
            pieces: [(Piece::None, Color::White); BOARD_SIZE],
            bitboards: [[0u64; 6]; 2],
            occupancies: [0u64; 2],

            zobrist: 0u64,
            en_passant_square: None,
            halfmove_clock: 0,
            castling_rights: Castling(0),
            side_to_move: Color::White,
        };

        if let Some(positioning_part) = tokens.next() {
            board.parse_positioning(positioning_part)?;
        } else {
            return Err("No piece placement part found");
        }

        board.side_to_move = match tokens.next().and_then(|s| s.chars().next()) {
            Some('w') => Color::White,
            Some('b') => Color::Black,
            _ => Color::White,
        };

        if let Some(castling_part) = tokens.next() {
            for chr in castling_part.chars() {
                board.castling_rights.0 |= match chr {
                    'K' => 1 << 0,
                    'Q' => 1 << 1,
                    'k' => 1 << 2,
                    'q' => 1 << 3,
                    _ => 0u8,
                }
            }
        }

        if let Some(en_passant_part) = tokens.next() {
            if en_passant_part.len() >= 2 {
                let mut en_passant_chars = en_passant_part.chars();
                let file_char = en_passant_chars.next().unwrap();
                let rank_char = en_passant_chars.next().unwrap();

                if file_char >= 'a' && file_char <= 'h' && rank_char >= '1' && rank_char <= '8' {
                    let file = (file_char as u8) - ('a' as u8);
                    let rank = rank_char.to_digit(10).unwrap() - 1;
                    let square = to_square(rank as i8, file as i8);
                    board.en_passant_square = Some(square);
                }
            }
        }

        if let Some(halfmove_clock_part) = tokens.next() {
            if let Ok(halfmove_clock) = halfmove_clock_part.parse::<u8>() {
                board.halfmove_clock = halfmove_clock;
            }
        }

        Ok(board)
    }
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

#[derive(Debug, PartialEq, Clone, Copy, Default)]
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
        let move_flag = crate::chess::attacks::tables::FLAGS_LUT[encoded_flags];

        debug_assert!(move_flag.move_type != MoveType::Invalid);
        move_flag
    }

    pub fn to_uci(self) -> String {
        let from_square = self.from_square();
        let to_square = self.to_square();

        let from_rank = from_square / BOARD_WIDTH as u8;
        let from_file = from_square % BOARD_WIDTH as u8;

        let to_rank = to_square / BOARD_WIDTH as u8;
        let to_file = to_square % BOARD_WIDTH as u8;

        let move_flags = self.get_flags();

        format!(
            "{}{}{}{}{}",
            (b'a' + from_file) as char,
            from_rank + 1,
            (b'a' + to_file) as char,
            to_rank + 1,
            move_flags.promotion.to_char()
        )
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

#[inline(always)]
pub fn bit(square: Square) -> u64 {
    1u64 << square
}

pub trait BitboardOnes: Sized + Copy {
    fn ones_iter(self) -> BitboardOnesIter;
}

pub struct BitboardOnesIter {
    bitboard: u64,
}

impl Iterator for BitboardOnesIter {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.bitboard == 0 {
            None
        } else {
            let sq = self.bitboard.trailing_zeros() as Square;
            self.bitboard &= self.bitboard - 1; // clear lowest set bit
            Some(sq)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let pop = self.bitboard.count_ones() as usize;
        (pop, Some(pop))
    }
}

impl ExactSizeIterator for BitboardOnesIter {}

impl BitboardOnes for u64 {
    fn ones_iter(self) -> BitboardOnesIter {
        BitboardOnesIter { bitboard: self }
    }
}
