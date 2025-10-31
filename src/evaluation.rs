use crate::chess::board::{Board, Color, PIECE_TYPES};

const PIECE_VALUES: [i16; PIECE_TYPES.len()] = [100, 320, 330, 500, 900, 20000];

// Provisional material only
impl Board {
    pub fn evaluate(&self) -> i16 {
        let mut score = 0i16;
        for piece_type in PIECE_TYPES {
            let w = self.bitboards[Color::White as usize][piece_type as usize].count_ones() as i16;
            let b = self.bitboards[Color::Black as usize][piece_type as usize].count_ones() as i16;
            score += (w - b) * PIECE_VALUES[piece_type as usize];
        }
        score
    }
}
