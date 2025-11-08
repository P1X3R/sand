use crate::chess::{zobrist::*, *};

pub struct Undo {
    mov: Move,
    captured: Piece,
    en_passant_square: Option<Square>,
    halfmove_clock: u8,
    castling_rights: u8, // 4 bits for KQkq
    zobrist: u64,
}

impl Board {
    #[inline(always)]
    fn update_rights_on_rook_change(&mut self, square: Square, color: Color) {
        self.castling_rights &= !(match (square, color) {
            (0, Color::White) => Castling::WQ,  // a1
            (7, Color::White) => Castling::WK,  // h1
            (56, Color::Black) => Castling::BQ, // a8
            (63, Color::Black) => Castling::BK, // h8
            _ => 0,
        });
    }

    /// Returns the square of the pawn captured by an en passant move
    /// `color` is the moving/attacker side's color
    #[inline(always)]
    fn get_en_passant_target(to: Square, color: Color) -> Square {
        match color {
            Color::White => to - BOARD_WIDTH as Square,
            Color::Black => to + BOARD_WIDTH as Square,
        }
    }

    /// This function doesn't update zobrist based on piece positioning because `toggle_piece`
    /// already does it
    #[inline(always)]
    fn update_zobrist(&mut self, old_en_passant: Option<Square>, old_rights: u8) {
        self.zobrist ^= *ZOBRIST_SIDE; // Side to move

        // En passant
        if old_en_passant != self.en_passant_square {
            self.zobrist ^= old_en_passant.map_or(0u64, |en_passant: Square| {
                ZOBRIST_EN_PASSANT[(en_passant % BOARD_WIDTH as Square) as usize]
            });
            self.zobrist ^= self.en_passant_square.map_or(0u64, |en_passant: Square| {
                ZOBRIST_EN_PASSANT[(en_passant % BOARD_WIDTH as Square) as usize]
            });
        }

        // Castling rights
        if old_rights != self.castling_rights {
            self.zobrist ^= ZOBRIST_CASTLING[old_rights as usize];
            self.zobrist ^= ZOBRIST_CASTLING[self.castling_rights as usize];
        }
    }

    /// Makes a move on the board, updating all internal state.
    /// Returns an `Undo` object that can restore the exact previous state.
    ///
    /// # Preconditions
    /// - `mov` must be a legal move in the current position
    #[inline(always)]
    pub fn make_move(&mut self, mov: Move) -> Undo {
        let from: Square = mov.get_from();
        let to: Square = mov.get_to();
        let flags: MoveFlag = mov.get_flags();
        let move_type: MoveType = flags.move_type;
        // `captured_color` is white if square is empty (captured_piece = Piece::None)
        let (captured_piece, captured_color): (Piece, Color) = self.pieces[to as usize];
        // I ignore the color since is the same to self.side_to_move
        let (piece_type, _): (Piece, Color) = self.pieces[from as usize];
        let final_type: Piece = if flags.promotion != Piece::None {
            flags.promotion
        } else {
            piece_type
        };
        let color: Color = self.side_to_move;
        let enemy: Color = color.toggle();
        let old_zobrist = self.zobrist;

        // Clear piece from original square
        self.toggle_piece(from, piece_type, color);

        // Handle special move types
        match move_type {
            MoveType::Capture => self.toggle_piece(to, captured_piece, captured_color),
            MoveType::EnPassantCapture => {
                self.toggle_piece(Board::get_en_passant_target(to, color), Piece::Pawn, enemy);
            }
            MoveType::KingSideCastle => {
                let (rook_from, rook_to) = match color {
                    Color::White => (7, 5),   // h1 -> f1
                    Color::Black => (63, 61), // h8 -> f1
                };
                self.toggle_piece(rook_from, Piece::Rook, color);
                self.toggle_piece(rook_to, Piece::Rook, color);
            }
            MoveType::QueenSideCastle => {
                let (rook_from, rook_to) = match color {
                    Color::White => (0, 3),   // a1 -> d1
                    Color::Black => (56, 59), // a8 -> d8
                };
                self.toggle_piece(rook_from, Piece::Rook, color);
                self.toggle_piece(rook_to, Piece::Rook, color);
            }
            _ => {}
        }

        // Land the moved piece
        self.toggle_piece(to, final_type, color);

        let old_en_passant = self.en_passant_square;
        self.en_passant_square = if move_type == MoveType::DoublePawnPush {
            Some(Board::get_en_passant_target(to, color))
        } else {
            None
        };

        let old_clock = self.halfmove_clock;
        if piece_type == Piece::Pawn || move_type == MoveType::Capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        };

        let old_rights = self.castling_rights;
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

        self.update_zobrist(old_en_passant, old_rights);

        Undo {
            mov,
            captured: captured_piece,
            en_passant_square: old_en_passant,
            halfmove_clock: old_clock,
            castling_rights: old_rights,
            zobrist: old_zobrist,
        }
    }

    /// Undo a move on the board, restoring to the previous state (before `make_move`)
    ///
    /// # Preconditions
    /// - `undo` from `make_move`
    #[inline(always)]
    pub fn undo_move(&mut self, undo: &Undo) {
        self.en_passant_square = undo.en_passant_square;
        self.halfmove_clock = undo.halfmove_clock;
        self.castling_rights = undo.castling_rights;
        self.side_to_move = self.side_to_move.toggle();

        let mov: Move = undo.mov;
        let from: Square = mov.get_from();
        let to: Square = mov.get_to();
        let flags: MoveFlag = mov.get_flags();
        let move_type: MoveType = flags.move_type;
        let (piece_type, _): (Piece, Color) = self.pieces[to as usize];
        let final_type: Piece = if flags.promotion != Piece::None {
            flags.promotion
        } else {
            piece_type
        };
        let initial_type: Piece = if flags.promotion != Piece::None {
            Piece::Pawn
        } else {
            piece_type
        };
        let color = self.side_to_move;

        // Clear the moved piece
        self.toggle_piece(to, final_type, color);

        // Handle special move types
        match move_type {
            MoveType::Capture => self.toggle_piece(to, undo.captured, color.toggle()),
            MoveType::EnPassantCapture => {
                self.toggle_piece(
                    Board::get_en_passant_target(to, color),
                    Piece::Pawn,
                    color.toggle(),
                );
            }
            MoveType::KingSideCastle => {
                let (rook_from, rook_to) = match color {
                    Color::White => (7, 5),   // h1 -> f1
                    Color::Black => (63, 61), // h8 -> f1
                };
                self.toggle_piece(rook_from, Piece::Rook, color);
                self.toggle_piece(rook_to, Piece::Rook, color);
            }
            MoveType::QueenSideCastle => {
                let (rook_from, rook_to) = match color {
                    Color::White => (0, 3),   // a1 -> d1
                    Color::Black => (56, 59), // a8 -> d8
                };
                self.toggle_piece(rook_from, Piece::Rook, color);
                self.toggle_piece(rook_to, Piece::Rook, color);
            }
            _ => {}
        }

        // Set the piece to its original square
        self.toggle_piece(from, initial_type, color);

        self.zobrist = undo.zobrist;
    }
}
