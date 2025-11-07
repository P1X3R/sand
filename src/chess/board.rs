use super::zobrist::*;
use crate::evaluation::W;

pub const BOARD_WIDTH: usize = 8;
pub const BOARD_SIZE: usize = 64;

pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub type Square = u8;
pub fn square_from_uci(uci: &str) -> Result<Square, &'static str> {
    let mut chars = uci.chars();

    if let (Some(file_char @ 'a'..='h'), Some(rank_char @ '1'..='8')) = (chars.next(), chars.next())
    {
        let file = (file_char as u8) - b'a';
        let rank = rank_char.to_digit(10).unwrap() - 1;
        let square = to_square(rank as i8, file as i8);

        return Ok(square);
    }

    Err("invalid character for square")
}

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

#[derive(Clone, Copy, PartialEq, Debug)]
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
            _ => Err("invalid character"),
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Piece::Pawn => 'p',
            Piece::Knight => 'n',
            Piece::Bishop => 'b',
            Piece::Rook => 'r',
            Piece::Queen => 'q',
            Piece::King => 'k',
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

#[derive(Clone, PartialEq, Debug)]
pub struct Board {
    pub pieces: [(Piece, Color); BOARD_SIZE],
    pub bitboards: [[u64; 6]; 2], // 6 piece types for 2 colors
    pub occupancies: [u64; 2],

    pub zobrist: u64,
    pub en_passant_square: Option<Square>,
    pub halfmove_clock: u8,
    pub castling_rights: u8, // 4 bits for KQkq
    pub side_to_move: Color,

    pub bonus: [W; 2],
    pub material: [i16; 2],
    pub phase: usize,
}

impl Board {
    /// Toggles the presence of a piece on a given square:
    /// - If the square is empty, the piece is added.
    /// - If the same piece/color is present, it is removed.
    ///
    /// Updates bitboards, occupancies, Zobrist and evaluation terms accordingly.
    #[inline(always)]
    pub fn toggle_piece(&mut self, square: Square, piece_type: Piece, color: Color) {
        let square_bit = bit(square);
        let (current_piece, current_color) = self.pieces[square as usize];

        debug_assert!(
            current_piece == Piece::None || (current_piece == piece_type && current_color == color),
            "toggle_piece mismatch at square {:?}, piece: {:?}, current piece: {:?}",
            square,
            (piece_type, color),
            (current_piece, current_color),
        );

        // mirror for whites because:
        // table index    -> 0=a8 63=h1
        // engine square  -> 0=a1 63=h8
        let square_lookup = match color {
            Color::White => square as usize ^ 56, // ^ 56 mirrors vertically
            Color::Black => square as usize,
        };

        if current_piece == Piece::None {
            self.phase += Board::PHASE_VALUE[piece_type as usize];
            self.bonus[color as usize] += Board::PST[piece_type as usize][square_lookup];
            self.material[color as usize] += Board::PIECE_VALUES[piece_type as usize];
            self.pieces[square as usize] = (piece_type, color)
        } else {
            self.phase -= Board::PHASE_VALUE[piece_type as usize];
            self.bonus[color as usize] -= Board::PST[piece_type as usize][square_lookup];
            self.material[color as usize] -= Board::PIECE_VALUES[piece_type as usize];
            self.pieces[square as usize] = (Piece::None, Color::White)
        };
        self.bitboards[color as usize][piece_type as usize] ^= square_bit;
        self.occupancies[color as usize] ^= square_bit;

        self.zobrist ^= ZOBRIST_PIECE[color as usize][piece_type as usize][square as usize];
    }

    /// This function doesn't update zobrist based on piece positioning because `toggle_piece`
    /// already does it
    fn set_zobrist_fen(&mut self) {
        if self.side_to_move == Color::Black {
            self.zobrist ^= *ZOBRIST_SIDE;
        }

        if let Some(en_passant_square) = self.en_passant_square {
            let en_passant_file = (en_passant_square % BOARD_WIDTH as Square) as usize;
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
                        return Err("too many ranks");
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
                        return Err("file out of bounds");
                    }
                    self.toggle_piece(to_square(rank as i8, file as i8), piece_type, color);
                    file += 1;
                }
            }
        }

        if rank != 0 || file != BOARD_WIDTH as u8 {
            return Err("incomplete board");
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

            bonus: [W(0, 0); 2],
            phase: 0,
            material: [0; 2],
        };

        if let Some(positioning_part) = tokens.next() {
            board.parse_positioning(positioning_part)?;
        } else {
            return Err("no piece placement part found");
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
            board.en_passant_square = square_from_uci(en_passant_part).ok();
        }

        if let Some(halfmove_clock_part) = tokens.next()
            && let Ok(halfmove_clock) = halfmove_clock_part.parse::<u8>()
        {
            board.halfmove_clock = halfmove_clock;
        }

        board.set_zobrist_fen();

        Ok(board)
    }

    /// Checks for insufficient material draws: KvK, KvN, KvB, and KvNN
    #[inline(always)]
    pub fn is_insufficient_material(&self) -> bool {
        let (mut pawn_rook_queen, mut bishop, mut knight) = (0u64, 0u64, 0u64);
        for color in 0..2 {
            pawn_rook_queen |= self.bitboards[color][Piece::Pawn as usize]
                | self.bitboards[color][Piece::Rook as usize]
                | self.bitboards[color][Piece::Queen as usize];
            bishop |= self.bitboards[color][Piece::Bishop as usize];
            knight |= self.bitboards[color][Piece::Knight as usize];
        }
        if pawn_rook_queen != 0 {
            return false;
        }

        let minors = bishop | knight;
        let either_bare = self.occupancies[Color::White as usize].count_ones() == 1
            || self.occupancies[Color::White as usize].count_ones() == 1;
        let have_one_minor = minors == 0 || minors & (minors - 1) == 0;

        either_bare && (have_one_minor || (bishop == 0 && knight.count_ones() <= 2))
    }

    #[inline(always)]
    pub fn is_fifty_move(&self) -> bool {
        self.halfmove_clock >= 100
    }

    pub fn calculate_zobrist(&self) -> u64 {
        let mut piece_zobrist = 0u64;
        for color in [Color::White, Color::Black] {
            for piece_type in PIECE_TYPES {
                let mut bitboard = self.bitboards[color as usize][piece_type as usize];
                while bitboard != 0 {
                    let square = bitboard.trailing_zeros();
                    piece_zobrist ^=
                        ZOBRIST_PIECE[color as usize][piece_type as usize][square as usize];
                    bitboard &= bitboard - 1;
                }
            }
        }

        let side_to_move_zobrist = if self.side_to_move == Color::Black {
            *ZOBRIST_SIDE
        } else {
            0u64
        };

        let en_passant_zobrist = if let Some(en_passant_square) = self.en_passant_square {
            let en_passant_file = en_passant_square % BOARD_WIDTH as Square;
            ZOBRIST_EN_PASSANT[en_passant_file as usize]
        } else {
            0u64
        };

        piece_zobrist
            ^ side_to_move_zobrist
            ^ ZOBRIST_CASTLING[self.castling_rights as usize]
            ^ en_passant_zobrist
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
