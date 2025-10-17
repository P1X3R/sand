use tinyvec::ArrayVec;

use crate::chess::{
    attacks::{
        magics,
        tables::{self, Offset},
    },
    board::*,
    moves::{Move, MoveFlag, MoveType},
};

pub const MAX_MOVES: usize = 256;

#[inline(always)]
pub fn gen_pawn_pushes(square: Square, occupancy: u64, color: Color) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    match color {
        Color::White => {
            let single: u64 = (bit(square) << BOARD_WIDTH) & !occupancy;
            let double: u64 = ((single & RANKS[2]) << BOARD_WIDTH) & !occupancy;

            single | double
        }
        Color::Black => {
            let single: u64 = (bit(square) >> BOARD_WIDTH) & !occupancy;
            let double: u64 = ((single & RANKS[5]) >> BOARD_WIDTH) & !occupancy;

            single | double
        }
    }
}

#[inline(always)]
fn gen_pawn_captures(square: Square, capturable: u64, color: Color) -> u64 {
    (match color {
        Color::White => tables::WPAWN_ATTACKS[square as usize],
        Color::Black => tables::BPAWN_ATTACKS[square as usize],
    }) & capturable
}

pub fn gen_jumping_attacks(square: Square, offsets: &[Offset]) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    let rank = square as i8 / BOARD_WIDTH as i8;
    let file = square as i8 % BOARD_WIDTH as i8;

    offsets.iter().fold(0u64, |attacks, offset| {
        let (r, f) = (rank + offset.rank, file + offset.file);
        if valid_axis(r) && valid_axis(f) {
            attacks | bit(to_square(r, f))
        } else {
            attacks
        }
    })
}

pub fn gen_edge_mask(square: Square) -> u64 {
    debug_assert!(square < BOARD_SIZE as Square);

    let bit: u64 = bit(square);

    const FILE_BB_1: u64 = 0x0101010101010101;
    const FILE_BB_8: u64 = 0x8080808080808080;

    [RANKS[0], RANKS[7], FILE_BB_1, FILE_BB_8]
        .iter()
        .fold(
            0u64,
            |mask, edge| if bit & edge == 0 { mask | edge } else { mask },
        )
}

pub fn gen_sliding_attacks(square: Square, occupancy: u64, directions: &[Offset]) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    let rank = square as i8 / BOARD_WIDTH as i8;
    let file = square as i8 % BOARD_WIDTH as i8;

    let mut attacks: u64 = 0;

    for offset in directions {
        let (mut attacked_rank, mut attacked_file) = (rank + offset.rank, file + offset.file);
        let mut ray: u64 = 0;

        while valid_axis(attacked_rank) && valid_axis(attacked_file) {
            ray |= bit(to_square(attacked_rank, attacked_file));

            if ray & occupancy != 0 {
                break;
            }

            attacked_rank += offset.rank;
            attacked_file += offset.file;
        }

        attacks |= ray;
    }

    attacks
}

// This code is textbook magic bitboards
#[inline(always)]
pub fn get_bishop_index(square: Square, occupancy: u64) -> usize {
    let magic = &magics::BISHOP_MAGICS[square as usize];
    let magic_index =
        (occupancy & tables::BISHOP_RM[square as usize]).wrapping_mul(magic.magic) >> magic.shift;
    debug_assert!(magic_index < (1 << tables::BISHOP_RM[square as usize].count_ones()));
    magic.offset + magic_index as usize
}

#[inline(always)]
pub fn get_rook_index(square: Square, occupancy: u64) -> usize {
    let magic = &magics::ROOK_MAGICS[square as usize];
    let magic_index =
        (occupancy & tables::ROOK_RM[square as usize]).wrapping_mul(magic.magic) >> magic.shift;
    debug_assert!(magic_index < (1 << tables::ROOK_RM[square as usize].count_ones()));
    magic.offset + magic_index as usize
}

#[inline(always)]
pub fn gen_piece_moves(square: Square, piece: Piece, color: Color, board: &Board) -> u64 {
    let friendly = board.occupancies[color as usize];
    let enemy = board.occupancies[color.toggle() as usize];
    let occupancy_all = friendly | enemy;

    debug_assert!(board.pieces[square as usize] == (piece, color));
    debug_assert!(friendly & bit(square) != 0);

    (match piece {
        Piece::Pawn => {
            // Include the en passant square as a potential target, since its capture is diagonal
            let en_passant_bit = board.en_passant_square.map_or(0u64, bit);
            let enemy_with_en_passant = en_passant_bit | enemy;

            gen_pawn_pushes(square, occupancy_all, color)
                | gen_pawn_captures(square, enemy_with_en_passant, color)
        }
        Piece::Knight => tables::KNIGHT_ATTACKS[square as usize],
        Piece::Bishop => magics::SLIDING_ATTACKS[get_bishop_index(square, occupancy_all)],
        Piece::Rook => magics::SLIDING_ATTACKS[get_rook_index(square, occupancy_all)],
        Piece::Queen => {
            magics::SLIDING_ATTACKS[get_bishop_index(square, occupancy_all)]
                | magics::SLIDING_ATTACKS[get_rook_index(square, occupancy_all)]
        }
        Piece::King => tables::KING_ATTACKS[square as usize],
        Piece::None => unreachable!("Tried to generate moves for an empty square"),
    }) & !friendly // You're not supposed to capture your own pieces
}

#[inline(always)]
fn get_move_type(piece: Piece, to_square: Square, from_square: Square, board: &Board) -> MoveType {
    if piece == Piece::Pawn && Some(to_square) == board.en_passant_square {
        return MoveType::EnPassantCapture;
    } else if board.pieces[to_square as usize].0 != Piece::None {
        return MoveType::Capture;
    } else if piece == Piece::Pawn && to_square.abs_diff(from_square) == (BOARD_WIDTH * 2) as u8 {
        return MoveType::DoublePawnPush;
    }
    MoveType::Quiet
}

#[inline(always)]
fn push_with_promotions(
    from_square: Square,
    to_square: Square,
    move_type: MoveType,
    piece: Piece,
    color: Color,
    move_list: &mut ArrayVec<[Move; MAX_MOVES]>,
) {
    let promotion_rank = match color {
        Color::White => RANKS[7],
        Color::Black => RANKS[0],
    };
    let is_promotion = piece == Piece::Pawn && bit(to_square) & promotion_rank != 0;

    if is_promotion {
        for promotion_piece in [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
            move_list.push(Move::new(
                from_square,
                to_square,
                MoveFlag {
                    move_type,
                    promotion: promotion_piece,
                },
            ));
        }
    } else {
        move_list.push(Move::new(
            from_square,
            to_square,
            MoveFlag {
                move_type,
                promotion: Piece::None,
            },
        ));
    }
}

pub fn gen_color_moves(board: &Board) -> ArrayVec<[Move; MAX_MOVES]> {
    let mut move_list = ArrayVec::<[Move; MAX_MOVES]>::new();
    let color = board.side_to_move;

    for piece_type in PIECE_TYPES {
        let bitboard = board.bitboards[color as usize][piece_type as usize];
        for from_square in bitboard.ones_iter() {
            let moves_bitboard = gen_piece_moves(from_square, piece_type, color, board);
            for to_square in moves_bitboard.ones_iter() {
                push_with_promotions(
                    from_square,
                    to_square,
                    get_move_type(piece_type, to_square, from_square, board),
                    piece_type,
                    color,
                    &mut move_list,
                );
            }
        }
    }

    move_list.extend(get_castling_moves(board));

    move_list
}

#[inline(always)]
pub fn get_attacker(square: Square, attacker_color: Color, board: &Board) -> u64 {
    let occupancy =
        board.occupancies[Color::White as usize] | board.occupancies[Color::Black as usize];
    let attacker_bitboards = board.bitboards[attacker_color as usize];

    let pawn_attacks = gen_pawn_captures(
        square,
        attacker_bitboards[Piece::Pawn as usize],
        attacker_color.toggle(),
    );
    let knight_attacks =
        tables::KNIGHT_ATTACKS[square as usize] & attacker_bitboards[Piece::Knight as usize];
    let bishop_rays = magics::SLIDING_ATTACKS[get_bishop_index(square, occupancy)];
    let bishop_queen_occupancy =
        attacker_bitboards[Piece::Bishop as usize] | attacker_bitboards[Piece::Queen as usize];
    let rook_rays = magics::SLIDING_ATTACKS[get_rook_index(square, occupancy)];
    let rook_queen_occupancy =
        attacker_bitboards[Piece::Rook as usize] | attacker_bitboards[Piece::Queen as usize];
    let king_attacks =
        tables::KING_ATTACKS[square as usize] & attacker_bitboards[Piece::King as usize];

    pawn_attacks
        | knight_attacks
        | (bishop_rays & bishop_queen_occupancy)
        | (rook_rays & rook_queen_occupancy)
        | king_attacks
}

#[inline(always)]
pub fn is_square_attacked(square: Square, attacker_color: Color, board: &Board) -> bool {
    let occupancy =
        board.occupancies[Color::White as usize] | board.occupancies[Color::Black as usize];
    let attacker_bitboards = board.bitboards[attacker_color as usize];
    let attackers_queens = attacker_bitboards[Piece::Queen as usize];

    gen_pawn_captures(
        square,
        attacker_bitboards[Piece::Pawn as usize],
        attacker_color.toggle(),
    ) != 0
        || (tables::KNIGHT_ATTACKS[square as usize] & attacker_bitboards[Piece::Knight as usize])
            != 0
        || (magics::SLIDING_ATTACKS[get_bishop_index(square, occupancy)]
            & (attacker_bitboards[Piece::Bishop as usize] | attackers_queens))
            != 0
        || (magics::SLIDING_ATTACKS[get_rook_index(square, occupancy)]
            & (attacker_bitboards[Piece::Rook as usize] | attackers_queens))
            != 0
        || (tables::KING_ATTACKS[square as usize] & attacker_bitboards[Piece::King as usize]) != 0
}

#[inline(always)]
fn get_castling_moves(board: &Board) -> ArrayVec<[Move; 2]> {
    const E1: Square = 4;
    const WHITE_KING_SIDE: Square = E1 + 2;
    const WHITE_QUEEN_SIDE: Square = E1 - 2;

    const E8: Square = 60;
    const BLACK_KING_SIDE: Square = E8 + 2;
    const BLACK_QUEEN_SIDE: Square = E8 - 2;

    const KING_SIDE_FLAG: MoveFlag = MoveFlag {
        move_type: MoveType::KingSideCastle,
        promotion: Piece::None,
    };
    const QUEEN_SIDE_FLAG: MoveFlag = MoveFlag {
        move_type: MoveType::QueenSideCastle,
        promotion: Piece::None,
    };

    let mut castles = ArrayVec::<[Move; 2]>::new();
    let occupancy =
        board.occupancies[Color::White as usize] | board.occupancies[Color::Black as usize];

    let rights = board.castling_rights;
    match board.side_to_move {
        Color::White => {
            // Check only if square is empty to be able to efficiently undo the move
            if rights & Castling::WK != 0 && occupancy & bit(WHITE_KING_SIDE) == 0 {
                castles.push(Move::new(E1, WHITE_KING_SIDE, KING_SIDE_FLAG));
            }
            if rights & Castling::WQ != 0 && occupancy & bit(WHITE_QUEEN_SIDE) == 0 {
                castles.push(Move::new(E1, WHITE_QUEEN_SIDE, QUEEN_SIDE_FLAG));
            }
        }
        Color::Black => {
            if rights & Castling::BK != 0 && occupancy & bit(BLACK_KING_SIDE) == 0 {
                castles.push(Move::new(E8, BLACK_KING_SIDE, KING_SIDE_FLAG));
            }
            if rights & Castling::BQ != 0 && occupancy & bit(BLACK_QUEEN_SIDE) == 0 {
                castles.push(Move::new(E8, BLACK_QUEEN_SIDE, QUEEN_SIDE_FLAG));
            }
        }
    };

    castles
}

/// The move must be already done in the board for this function to work properly
#[inline(always)]
pub fn is_legal_move(mov: Move, board: &Board) -> bool {
    let move_type = mov.get_flags().move_type;
    let color = board.side_to_move;
    let king_bitboard = board.bitboards[color as usize][Piece::King as usize];

    // Move must be already done
    debug_assert!(
        board.pieces[mov.get_from() as usize].0 == Piece::None
            && board.pieces[mov.get_to() as usize].0 != Piece::None
    );

    if move_type == MoveType::KingSideCastle || move_type == MoveType::QueenSideCastle {
        let (in_between, through, rook_bit) = match (color, move_type) {
            (Color::White, MoveType::KingSideCastle) => (&[5, 6][..], &[4, 5, 6][..], bit(5)),
            (Color::White, MoveType::QueenSideCastle) => (&[1, 2, 3][..], &[4, 3, 2][..], bit(3)),
            (Color::Black, MoveType::KingSideCastle) => (&[61, 62][..], &[60, 61, 62][..], bit(61)),
            (Color::Black, MoveType::QueenSideCastle) => {
                (&[57, 58, 59][..], &[60, 59, 58][..], bit(59))
            }
            _ => unreachable!(),
        };

        let occupancy =
            board.occupancies[Color::White as usize] | board.occupancies[Color::Black as usize];
        let occupancy_without_updated_pieces = occupancy & !(king_bitboard | rook_bit);

        through
            .iter()
            .all(|&square| !is_square_attacked(square, color.toggle(), board))
            && in_between
                .iter()
                .all(|&square| occupancy_without_updated_pieces & bit(square) == 0)
    } else {
        !is_square_attacked(
            king_bitboard.trailing_zeros() as Square,
            color.toggle(),
            board,
        )
    }
}
