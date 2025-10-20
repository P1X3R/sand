use crate::chess::board::{Board, Color, PIECE_TYPES};

const PIECE_VALUES: [i16; PIECE_TYPES.len() + 1] = [100, 320, 330, 500, 900, 20000, 0];

// Provisional material only
impl Board {
    pub fn evaluate(&self) -> i16 {
        self.pieces.iter().fold(0i16, |score, (piece_type, color)| {
            score
                + match color {
                    Color::White => PIECE_VALUES[*piece_type as usize],
                    Color::Black => -PIECE_VALUES[*piece_type as usize],
                }
        })
    }
}
