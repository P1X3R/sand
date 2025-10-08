use crate::chess::{
    attacks::{
        magics,
        tables::{self, Offset},
    },
    board::*,
};

pub const MAX_MOVES: usize = 256;

pub fn gen_pawn_pushes(square: Square, occupancy: u64, color: Color) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    let pawn_bit: u64 = 1u64 << square;

    match color {
        Color::White => {
            let single: u64 = (pawn_bit << BOARD_WIDTH) & !occupancy;
            let double: u64 = ((single & RANKS[2]) << BOARD_WIDTH) & !occupancy;

            single | double
        }
        Color::Black => {
            let single: u64 = (pawn_bit >> BOARD_WIDTH) & !occupancy;
            let double: u64 = ((single & RANKS[5]) >> BOARD_WIDTH) & !occupancy;

            single | double
        }
    }
}

pub fn gen_jumping_attacks(square: Square, offsets: &[Offset]) -> u64 {
    debug_assert!(square < BOARD_SIZE as u8);

    let rank = square as i8 / BOARD_WIDTH as i8;
    let file = square as i8 % BOARD_WIDTH as i8;

    offsets.iter().fold(0u64, |attacks, offset| {
        let (r, f) = (rank + offset.rank, file + offset.file);
        if valid_axis(r) && valid_axis(f) {
            attacks | (1u64 << to_square(r, f))
        } else {
            attacks
        }
    })
}

pub fn gen_edge_mask(square: usize) -> u64 {
    debug_assert!(square < BOARD_SIZE);

    let bit: u64 = 1u64 << square;

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
        let (mut r, mut f) = (rank + offset.rank, file + offset.file);
        let mut ray: u64 = 0;

        while valid_axis(r) && valid_axis(f) {
            ray |= 1u64 << to_square(r, f);

            if ray & occupancy != 0 {
                break;
            }

            r += offset.rank;
            f += offset.file;
        }

        attacks |= ray;
    }

    attacks
}

pub fn get_occupancy(mut variant: usize, mut relevant_mask: u64) -> u64 {
    debug_assert!(variant < (1 << relevant_mask.count_ones()));

    let mut occupancy: u64 = 0;

    while variant != 0 {
        if variant & 1 != 0 {
            occupancy |= relevant_mask & relevant_mask.wrapping_neg();
        }

        variant >>= 1;
        relevant_mask &= relevant_mask - 1;
    }

    occupancy
}

#[inline(always)]
pub fn get_bishop_index(square: Square, occupancy: u64) -> usize {
    let magic = &magics::BISHOP_MAGICS[square as usize];
    let variant =
        (occupancy & tables::BISHOP_RM[square as usize]).wrapping_mul(magic.magic) >> magic.shift;
    debug_assert!(variant < (1 << tables::BISHOP_RM[square as usize].count_ones()));
    magic.offset + variant as usize
}

#[inline(always)]
pub fn get_rook_index(square: Square, occupancy: u64) -> usize {
    let magic = &magics::ROOK_MAGICS[square as usize];
    let variant =
        (occupancy & tables::ROOK_RM[square as usize]).wrapping_mul(magic.magic) >> magic.shift;
    debug_assert!(variant < (1 << tables::ROOK_RM[square as usize].count_ones()));
    magic.offset + variant as usize
}

#[inline(always)]
pub fn gen_piece_moves(square: Square, piece: Piece, color: Color, board: &Board) -> u64 {
    let friendly = board.occupancies[color as usize];
    let enemy = board.occupancies[color.toggle() as usize];
    let occupancy_all = friendly | enemy;

    debug_assert!(board.pieces[square as usize] == (piece, color));
    debug_assert!(friendly & (1u64 << square) != 0);

    match piece {
        Piece::Pawn => {
            // Include the en passant square as a potential target, since its capture is diagonal
            let en_passant_bit = board.en_passant_square.map_or(0, |square| 1u64 << square);
            let occupancy_with_en_passant = en_passant_bit | enemy;
            let attacks = match color {
                Color::White => tables::WPAWN_ATTACKS[square as usize],
                Color::Black => tables::BPAWN_ATTACKS[square as usize],
            } & occupancy_with_en_passant;

            gen_pawn_pushes(square, occupancy_all, color) | attacks
        }
        Piece::Knight => tables::KNIGHT_ATTACKS[square as usize] & !friendly,
        Piece::Bishop => {
            magics::SLIDING_ATTACKS[get_bishop_index(square, occupancy_all)] & !friendly
        }
        Piece::Rook => magics::SLIDING_ATTACKS[get_rook_index(square, occupancy_all)] & !friendly,
        Piece::Queen => {
            (magics::SLIDING_ATTACKS[get_bishop_index(square, occupancy_all)]
                | magics::SLIDING_ATTACKS[get_rook_index(square, occupancy_all)])
                & !friendly
        }
        Piece::King => tables::KING_ATTACKS[square as usize] & !friendly,
        Piece::None => unreachable!("Tried to generate moves for an empty square"),
    }
}

pub fn gen_color_moves(board: &Board) -> tinyvec::ArrayVec<[Move; MAX_MOVES]> {
    let mut move_list = tinyvec::ArrayVec::<[Move; 256]>::new();
    let color = board.side_to_move;

    for piece in PIECE_TYPES {
        let mut bitboard: u64 = board.bitboards[color as usize][piece as usize];
        while bitboard != 0 {
            let from_square: Square = bitboard.trailing_zeros() as u8;
            let mut moves_bits: u64 = gen_piece_moves(from_square, piece, color, board);

            while moves_bits != 0 {
                let to_square: Square = moves_bits.trailing_zeros() as u8;
                let move_type =
                    if piece == Piece::Pawn && Some(to_square) == board.en_passant_square {
                        MoveType::EnPassantCapture
                    } else if board.pieces[to_square as usize].0 != Piece::None {
                        MoveType::Capture
                    } else if piece == Piece::Pawn
                        && to_square.abs_diff(from_square) == (BOARD_WIDTH * 2) as u8
                    {
                        MoveType::DoublePawnPush
                    } else {
                        MoveType::Quiet
                    };
                let promotion_rank = match color {
                    Color::White => RANKS[7],
                    Color::Black => RANKS[0],
                };
                let is_promotion =
                    piece == Piece::Pawn && (1u64 << to_square) & promotion_rank != 0;

                if is_promotion {
                    const PROMOTION_TYPES: [Piece; 4] =
                        [Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen];
                    for promotion_piece in PROMOTION_TYPES {
                        move_list.push(Move::new(
                            from_square,
                            to_square,
                            MoveFlag {
                                move_type: move_type,
                                promotion: promotion_piece,
                            },
                        ));
                    }
                } else {
                    move_list.push(Move::new(
                        from_square,
                        to_square,
                        MoveFlag {
                            move_type: move_type,
                            promotion: Piece::None,
                        },
                    ));
                }

                moves_bits &= moves_bits - 1;
            }

            bitboard &= bitboard - 1;
        }
    }

    move_list
}
