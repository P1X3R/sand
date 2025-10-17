use crate::chess::{
    board::{BOARD_WIDTH, Board, Castling, Color, Piece, Square},
    moves::{Move, MoveFlag, MoveType},
    zobrist::{ZOBRIST_CASTLING, ZOBRIST_EN_PASSANT, ZOBRIST_SIDE},
};

impl Board {
    fn update_rights_on_rook_change(&mut self, square: Square, color: Color) {
        self.castling_rights &= !(match (square, color) {
            (0, Color::White) => Castling::WQ,
            (7, Color::White) => Castling::WK,
            (56, Color::Black) => Castling::BQ,
            (63, Color::Black) => Castling::BK,
            _ => 0,
        });
    }

    pub fn make_move(&mut self, mov: Move) {
        let from: Square = mov.get_from();
        let to: Square = mov.get_to();
        let flags: MoveFlag = mov.get_flags();
        let move_type: MoveType = flags.move_type;
        // `captured_color` is white if is no capture
        let (captured_piece, captured_color): (Piece, Color) = self.pieces[to as usize];
        let (piece_type, _): (Piece, Color) = self.pieces[from as usize];
        let final_type: Piece = if flags.promotion != Piece::None {
            flags.promotion
        } else {
            piece_type
        };
        let color: Color = self.side_to_move;
        let enemy: Color = color.toggle();

        // Clear piece from original square
        self.toggle_piece(from, piece_type, color);

        // Handle special move types
        match move_type {
            MoveType::Capture => self.toggle_piece(to, captured_piece, captured_color),
            MoveType::EnPassantCapture => {
                let captured_pawn_square = match color {
                    Color::White => to - BOARD_WIDTH as Square,
                    Color::Black => to + BOARD_WIDTH as Square,
                };
                self.toggle_piece(captured_pawn_square, Piece::Pawn, enemy);
            }
            MoveType::KingSideCastle => {
                let (rook_from, rook_to) = match color {
                    Color::White => (7, 5),
                    Color::Black => (63, 61),
                };
                self.toggle_piece(rook_from, Piece::Rook, color);
                self.toggle_piece(rook_to, Piece::Rook, color);
            }
            MoveType::QueenSideCastle => {
                let (rook_from, rook_to) = match color {
                    Color::White => (0, 3),
                    Color::Black => (56, 59),
                };
                self.toggle_piece(rook_from, Piece::Rook, color);
                self.toggle_piece(rook_to, Piece::Rook, color);
            }
            _ => {}
        }

        // Land the moved piece
        self.toggle_piece(to, final_type, color);

        let initial_en_passant = self.en_passant_square;
        self.en_passant_square = if move_type == MoveType::DoublePawnPush {
            Some(to)
        } else {
            None
        };

        if piece_type == Piece::Pawn || move_type == MoveType::Capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        };

        let initial_rights = self.castling_rights;
        if piece_type == Piece::King {
            self.castling_rights &= !(match color {
                Color::White => Castling::WK | Castling::WQ,
                Color::Black => Castling::BK | Castling::BQ,
            });
        } else if piece_type == Piece::Rook {
            self.update_rights_on_rook_change(from, color);
        }
        if captured_piece == Piece::Rook {
            self.update_rights_on_rook_change(to, enemy);
        }

        self.side_to_move = enemy;

        self.zobrist ^= *ZOBRIST_SIDE;
        if initial_en_passant != self.en_passant_square {
            self.zobrist ^= initial_en_passant.map_or(0u64, |en_passant: Square| {
                ZOBRIST_EN_PASSANT[(en_passant / BOARD_WIDTH as Square) as usize]
            });
            self.zobrist ^= self.en_passant_square.map_or(0u64, |en_passant: Square| {
                ZOBRIST_EN_PASSANT[(en_passant / BOARD_WIDTH as Square) as usize]
            });
        }
        if initial_rights != self.castling_rights {
            self.zobrist ^= ZOBRIST_CASTLING[initial_rights as usize];
            self.zobrist ^= ZOBRIST_CASTLING[self.castling_rights as usize];
        }
    }
}
