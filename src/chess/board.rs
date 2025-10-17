use super::zobrist::*;

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

pub struct Castling;
impl Castling {
    pub const WK: u8 = 1;
    pub const WQ: u8 = 2;
    pub const BK: u8 = 4;
    pub const BQ: u8 = 8;
}

#[derive(Clone)]
pub struct Board {
    pub pieces: [(Piece, Color); BOARD_SIZE],
    pub bitboards: [[u64; 6]; 2], // 6 piece types for 2 colors
    pub occupancies: [u64; 2],

    pub zobrist: u64,
    pub en_passant_square: Option<Square>,
    pub halfmove_clock: u8,
    pub castling_rights: u8, // 4 bits for KQkq
    pub side_to_move: Color,
}

impl Board {
    /// Toggles the presence of a piece on a given square:
    /// - If the square is empty, the piece is added.
    /// - If the same piece/color is present, it is removed.
    /// Updates bitboards, occupancies, and Zobrist accordingly.
    #[inline(always)]
    pub fn toggle_piece(&mut self, square: Square, piece_type: Piece, color: Color) {
        let square_bit = bit(square);
        let (current_piece, current_color) = self.pieces[square as usize];

        debug_assert!(
            current_piece == Piece::None || (current_piece == piece_type && current_color == color),
            "toggle_piece mismatch at square {:?}",
            square
        );

        self.pieces[square as usize] = if current_piece == Piece::None {
            (piece_type, color)
        } else {
            (Piece::None, Color::White)
        };
        self.bitboards[color as usize][piece_type as usize] ^= square_bit;
        self.occupancies[color as usize] ^= square_bit;

        self.zobrist ^= ZOBRIST_PIECE[color as usize][piece_type as usize][square as usize];
    }

    /// This function doesn't update zobrist based on piece positioning because `toggle_piece`
    /// already does it
    fn update_zobrist(&mut self) {
        if self.side_to_move == Color::Black {
            self.zobrist ^= *ZOBRIST_SIDE;
        }
        if let Some(en_passant_square) = self.en_passant_square {
            let en_passant_file = (en_passant_square / BOARD_WIDTH as Square) as usize;
            self.zobrist ^= ZOBRIST_EN_PASSANT[en_passant_file];
        }
        self.zobrist ^= ZOBRIST_CASTLING[self.castling_rights as usize];
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

    pub fn new(fen: &str) -> Result<Self, &'static str> {
        let mut tokens = fen.split_whitespace();

        let mut board = Board {
            pieces: [(Piece::None, Color::White); BOARD_SIZE],
            bitboards: [[0u64; 6]; 2],
            occupancies: [0u64; 2],

            zobrist: 0u64,
            en_passant_square: None,
            halfmove_clock: 0,
            castling_rights: 0,
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
                board.castling_rights |= match chr {
                    'K' => 1 << 0,
                    'Q' => 1 << 1,
                    'k' => 1 << 2,
                    'q' => 1 << 3,
                    _ => 0u8,
                }
            }
        }

        if let Some(en_passant_part) = tokens.next() {
            let mut en_passant_chars = en_passant_part.chars();

            if let (Some(file_char @ 'a'..='h'), Some(rank_char @ '1'..='8')) =
                (en_passant_chars.next(), en_passant_chars.next())
            {
                let file = (file_char as u8) - b'a';
                let rank = rank_char.to_digit(10).unwrap() - 1;
                let square = to_square(rank as i8, file as i8);
                board.en_passant_square = Some(square);
            }
        }

        if let Some(halfmove_clock_part) = tokens.next()
            && let Ok(halfmove_clock) = halfmove_clock_part.parse::<u8>()
        {
            board.halfmove_clock = halfmove_clock;
        }

        board.update_zobrist();

        Ok(board)
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
