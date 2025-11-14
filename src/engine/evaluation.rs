use crate::chess::*;
use std::ops::{AddAssign, SubAssign};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct W(pub i16, pub i16);

impl AddAssign for W {
    fn add_assign(&mut self, rhs: Self) {
        *self = W(self.0 + rhs.0, self.1 + rhs.1);
    }
}
impl SubAssign for W {
    fn sub_assign(&mut self, rhs: Self) {
        *self = W(self.0 - rhs.0, self.1 - rhs.1);
    }
}

impl Board {
    // len is piece types and nothing
    pub const PIECE_VALUES: [i16; PIECE_TYPES.len() + 1] = [100, 320, 330, 500, 900, 20000, 0];
    pub const PHASE_VALUE: [usize; PIECE_TYPES.len()] = [0, 1, 1, 2, 4, 0];
    const TOTAL_PHASE: usize = Board::PHASE_VALUE[Piece::Pawn as usize] * 16
        + Board::PHASE_VALUE[Piece::Knight as usize] * 4
        + Board::PHASE_VALUE[Piece::Bishop as usize] * 4
        + Board::PHASE_VALUE[Piece::Rook as usize] * 4
        + Board::PHASE_VALUE[Piece::Queen as usize] * 2;
    const PHASE_SCALE: usize = 256;

    // stolen from PeSTO
#[rustfmt::skip]
    pub const PST: [[W; BOARD_SIZE]; PIECE_TYPES.len()] = [
        // pawn
        [
            W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0),
            W(98, 178), W(134, 173), W(61, 158), W(95, 134), W(68, 147), W(126, 132), W(34, 165), W(-11, 187),
            W(-6, 94), W(7, 100), W(26, 85), W(31, 67), W(65, 56), W(56, 53), W(25, 82), W(-20, 84),
            W(-14, 32), W(13, 24), W(6, 13), W(21, 5), W(23, -2), W(12, 4), W(17, 17), W(-23, 17),
            W(-27, 13), W(-2, 9), W(-5, -3), W(12, -7), W(17, -7), W(6, -8), W(10, 3), W(-25, -1),
            W(-26, 4), W(-4, 7), W(-4, -6), W(-10, 1), W(3, 0), W(3, -5), W(33, -1), W(-12, -8),
            W(-35, 13), W(-1, 8), W(-20, 8), W(-23, 10), W(-15, 13), W(24, 0), W(38, 2), W(-22, -7),
            W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0), W(0, 0),
        ],
        // knight
        [
            W(-167, -58), W(-89, -38), W(-34, -13), W(-49, -28), W(61, -31), W(-97, -27), W(-15, -63), W(-107, -99),
            W(-73, -25), W(-41, -8), W(72, -25), W(36, -2), W(23, -9), W(62, -25), W(7, -24), W(-17, -52),
            W(-47, -24), W(60, -20), W(37, 10), W(65, 9), W(84, -1), W(129, -9), W(73, -19), W(44, -41),
            W(-9, -17), W(17, 3), W(19, 22), W(53, 22), W(37, 22), W(69, 11), W(18, 8), W(22, -18),
            W(-13, -18), W(4, -6), W(16, 16), W(13, 25), W(28, 16), W(19, 17), W(21, 4), W(-8, -18),
            W(-23, -23), W(-9, -3), W(12, -1), W(10, 15), W(19, 10), W(17, -3), W(25, -20), W(-16, -22),
            W(-29, -42), W(-53, -20), W(-12, -10), W(-3, -5), W(-1, -2), W(18, -20), W(-14, -23), W(-19, -44),
            W(-105, -29), W(-21, -51), W(-58, -23), W(-33, -15), W(-17, -22), W(-28, -18), W(-19, -50), W(-23, -64),
        ],
        // bishop
        [
            W(-29, -14), W(4, -21), W(-82, -11), W(-37, -8), W(-25, -7), W(-42, -9), W(7, -17), W(-8, -24),
            W(-26, -8), W(16, -4), W(-18, 7), W(-13, -12), W(30, -3), W(59, -13), W(18, -4), W(-47, -14),
            W(-16, 2), W(37, -8), W(43, 0), W(40, -1), W(35, -2), W(50, 6), W(37, 0), W(-2, 4),
            W(-4, -3), W(5, 9), W(19, 12), W(50, 9), W(37, 14), W(37, 10), W(7, 3), W(-2, 2),
            W(-6, -6), W(13, 3), W(13, 13), W(26, 19), W(34, 7), W(12, 10), W(10, -3), W(4, -9),
            W(0, -12), W(15, -3), W(15, 8), W(15, 10), W(14, 13), W(27, 3), W(18, -7), W(10, -15),
            W(4, -14), W(15, -18), W(16, -7), W(0, -1), W(7, 4), W(21, -9), W(33, -15), W(1, -27),
            W(-33, -23), W(-3, -9), W(-14, -23), W(-21, -5), W(-13, -9), W(-12, -16), W(-39, -5), W(-21, -17),
        ],
        // rook
        [
            W(32, 13), W(42, 10), W(32, 18), W(51, 15), W(63, 12), W(9, 12), W(31, 8), W(43, 5),
            W(27, 11), W(32, 13), W(58, 13), W(62, 11), W(80, -3), W(67, 3), W(26, 8), W(44, 3),
            W(-5, 7), W(19, 7), W(26, 7), W(36, 5), W(17, 4), W(45, -3), W(61, -5), W(16, -3),
            W(-24, 4), W(-11, 3), W(7, 13), W(26, 1), W(24, 2), W(35, 1), W(-8, -1), W(-20, 2),
            W(-36, 3), W(-26, 5), W(-12, 8), W(-1, 4), W(9, -5), W(-7, -6), W(6, -8), W(-23, -11),
            W(-45, -4), W(-25, 0), W(-16, -5), W(-17, -1), W(3, -7), W(0, -12), W(-5, -8), W(-33, -16),
            W(-44, -6), W(-16, -6), W(-20, 0), W(-9, 2), W(-1, -9), W(11, -9), W(-6, -11), W(-71, -3),
            W(-19, -9), W(-13, 2), W(1, 3), W(17, -1), W(16, -5), W(7, -13), W(-37, 4), W(-26, -20),
        ],
        // queen
        [
            W(-28, -9), W(0, 22), W(29, 22), W(12, 27), W(59, 27), W(44, 19), W(43, 10), W(45, 20),
            W(-24, -17), W(-39, 20), W(-5, 32), W(1, 41), W(-16, 58), W(57, 25), W(28, 30), W(54, 0),
            W(-13, -20), W(-17, 6), W(7, 9), W(8, 49), W(29, 47), W(56, 35), W(47, 19), W(57, 9),
            W(-27, 3), W(-27, 22), W(-16, 24), W(-16, 45), W(-1, 57), W(17, 40), W(-2, 57), W(1, 36),
            W(-9, -18), W(-26, 28), W(-9, 19), W(-10, 47), W(-2, 31), W(-4, 34), W(3, 39), W(-3, 23),
            W(-14, -16), W(2, -27), W(-11, 15), W(-2, 6), W(-5, 9), W(2, 17), W(14, 10), W(5, 5),
            W(-35, -22), W(-8, -23), W(11, -30), W(2, -16), W(8, -16), W(15, -23), W(-3, -36), W(1, -32),
            W(-1, -33), W(-18, -28), W(-9, -22), W(10, -43), W(-15, -5), W(-25, -32), W(-31, -20), W(-50, -41),
        ],
        // king
        [
            W(-65, -74), W(23, -35), W(16, -18), W(-15, -18), W(-56, -11), W(-34, 15), W(2, 4), W(13, -17),
            W(29, -12), W(-1, 17), W(-20, 14), W(-7, 17), W(-8, 17), W(-4, 38), W(-38, 23), W(-29, 11),
            W(-9, 10), W(24, 17), W(2, 23), W(-16, 15), W(-20, 20), W(6, 45), W(22, 44), W(-22, 13),
            W(-17, -8), W(-20, 22), W(-12, 24), W(-27, 27), W(-30, 26), W(-25, 33), W(-14, 26), W(-36, 3),
            W(-49, -18), W(-1, -4), W(-27, 21), W(-39, 24), W(-46, 27), W(-44, 23), W(-33, 9), W(-51, -11),
            W(-14, -19), W(-14, -3), W(-22, 11), W(-46, 21), W(-44, 23), W(-30, 16), W(-15, 7), W(-27, -9),
            W(1, -27), W(7, -11), W(-8, 4), W(-64, 13), W(-43, 14), W(-16, 4), W(9, -5), W(8, -17),
            W(-15, -53), W(36, -34), W(12, -21), W(-54, -11), W(8, -28), W(-28, -14), W(24, -24), W(14, -43),
        ],
    ];

    fn calculate_bonus(&self) -> [W; 2] {
        [Color::White, Color::Black].map(|color| {
            self.occupancies[color as usize]
                .ones_iter()
                .filter_map(|square| {
                    let (piece_type, piece_color) = self.pieces[square as usize];
                    (piece_color == color && piece_type != Piece::None).then_some(
                        Board::PST[piece_type as usize][match color {
                            // mirror for whites because:
                            // table index    -> 0=a8 63=h1
                            // engine square  -> 0=a1 63=h8
                            Color::White => square as usize ^ 56, // ^ 56 mirrors vertically
                            Color::Black => square as usize,
                        }],
                    )
                })
                .fold(W(0, 0), |acc, curr| W(acc.0 + curr.0, acc.1 + curr.1))
        })
    }

    /// from whites perspective in centipawns
    pub fn evaluate(&self) -> i16 {
        debug_assert_eq!(self.bonus, self.calculate_bonus(), "bonus mismatch");

        let material_score =
            self.material[Color::White as usize] - self.material[Color::Black as usize];

        let phase_ratio = ((self.phase * Board::PHASE_SCALE + (Board::TOTAL_PHASE / 2))
            / Board::TOTAL_PHASE) as i32;

        let midgame_bonus: i32 =
            (self.bonus[Color::White as usize].0 - self.bonus[Color::Black as usize].0) as i32;

        let endgame_bonus =
            (self.bonus[Color::White as usize].1 - self.bonus[Color::Black as usize].1) as i32;

        let positional = ((midgame_bonus * phase_ratio)
            + (endgame_bonus * (Board::PHASE_SCALE as i32 - phase_ratio)))
            / Board::PHASE_SCALE as i32;

        (positional as i16) + material_score
    }
}
