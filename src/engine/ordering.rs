use std::{
    array,
    sync::{
        LazyLock,
        atomic::{AtomicI16, Ordering},
    },
};

use crate::{chess::*, engine::search::Searcher};
use tinyvec::ArrayVec;

pub(crate) type ScoredMoveList = ArrayVec<[(Move, i16); MAX_MOVES]>;
pub(crate) struct SearchContext<'a> {
    pub board: &'a Board,
    pub pv_line: &'a [Move],
    pub killers: &'a [[Option<Move>; 2]; Searcher::MAX_PLY],
    pub history_heuristic: &'a HistoryHeuristics,
    pub hash_move: Option<Move>,
    pub ply: usize,
}

struct MoveBuckets;
impl MoveBuckets {
    pub const CAPTURES_PROMOTIONS: i16 = 10_000;
    pub const KILLERS: i16 = 5_000;
    pub const UNDER_PROMOTIONS: i16 = 2_000;
}

// subtract the king because `Board::PIECE_VALUES[Piece::KING as usize] = 20000`, just to avoid
// overflow. Can't capture king anyways
static MVV_LVA: LazyLock<[[i16; PIECE_TYPES.len()]; PIECE_TYPES.len() - 1]> = LazyLock::new(|| {
    use std::array::from_fn;
    from_fn(|victim: usize| {
        from_fn(|attacker: usize| 10 * Board::PIECE_VALUES[victim] - Board::PIECE_VALUES[attacker])
    })
});

fn get_least_valuable_attacker(
    attackers: u64,
    board: &Board,
    side_to_move: Color,
) -> Option<(u64, Piece)> {
    for piece_type in [
        Piece::Pawn,
        Piece::Knight,
        Piece::Bishop,
        Piece::Rook,
        Piece::Queen,
        Piece::King,
    ] {
        let simulated_attackers =
            board.bitboards[side_to_move as usize][piece_type as usize] & attackers;
        if simulated_attackers != 0 {
            return Some((
                simulated_attackers & simulated_attackers.wrapping_neg(), // isolate the lsb
                piece_type,
            ));
        }
    }

    return None;
}

fn consider_x_rays(square: Square, side_to_move: Color, occupancy: u64, board: &Board) -> u64 {
    use crate::chess::attacks::magics::SLIDING_ATTACKS;

    let attacker_bitboards = board.bitboards[side_to_move as usize];

    let bishop_rays = SLIDING_ATTACKS[get_bishop_index(square, occupancy)];
    let rook_rays = SLIDING_ATTACKS[get_rook_index(square, occupancy)];

    let bishop_queen_occupancy =
        attacker_bitboards[Piece::Bishop as usize] | attacker_bitboards[Piece::Queen as usize];
    let rook_queen_occupancy =
        attacker_bitboards[Piece::Rook as usize] | attacker_bitboards[Piece::Queen as usize];

    ((bishop_rays & bishop_queen_occupancy) | (rook_rays & rook_queen_occupancy)) & occupancy
}

pub fn can_prune_by_see(mov: Move, board: &Board) -> bool {
    let flags = mov.get_flags();
    if flags.move_type != MoveType::Capture && flags.move_type != MoveType::EnPassantCapture {
        return false; // don't prune non-captures by SEE
    }

    let from: Square = mov.get_from();
    let to: Square = mov.get_to();
    let (victim, _): (Piece, Color) = board.pieces[to as usize];
    let (attacker, _): (Piece, Color) = board.pieces[from as usize];
    let attacker = if flags.promotion != Piece::None {
        flags.promotion
    } else {
        attacker
    };

    !see_ge((from, attacker), (to, victim), board, 0)
}

/// inspired from Stockfish implementation
fn see_ge(
    (from, initial_attacker): (Square, Piece),
    (target, initial_victim): (Square, Piece),
    board: &Board,
    threshold: i16,
) -> bool {
    let mut swap = Board::PIECE_VALUES[initial_victim as usize] - threshold;
    if swap < 0 {
        return false;
    }

    swap = Board::PIECE_VALUES[initial_attacker as usize] - swap;
    if swap <= 0 {
        return true;
    }

    let may_x_ray: u64 = [Piece::Pawn, Piece::Bishop, Piece::Rook, Piece::Queen]
        .iter()
        .fold(0, |acc, &piece_type| {
            acc | board.bitboards[0][piece_type as usize] | board.bitboards[1][piece_type as usize]
        });
    let occupancy: u64 = board.occupancies[0] | board.occupancies[1]; // for both colors
    let mut occupancy = occupancy ^ bit(from); // remove first attacker

    let mut side_to_move = board.side_to_move.toggle();
    let mut attackers = get_attackers(target, side_to_move, board) & occupancy;
    let mut side_has_advantage = true;

    while let Some((attacker, attacker_type)) =
        get_least_valuable_attacker(attackers, board, side_to_move)
    {
        if attacker_type == Piece::King {
            return if attackers & occupancy != 0 {
                !side_has_advantage
            } else {
                side_has_advantage
            };
        }

        side_has_advantage = !side_has_advantage;

        swap = Board::PIECE_VALUES[attacker_type as usize] - swap;
        if swap < (side_has_advantage as i16) {
            break;
        }

        occupancy ^= attacker;
        attackers ^= attacker;

        if attacker & may_x_ray != 0 {
            attackers |= consider_x_rays(target, side_to_move, occupancy, board)
        }

        side_to_move = side_to_move.toggle();
    }

    side_has_advantage
}

fn score_move(mov: Move, search_ctx: &SearchContext) -> i16 {
    if search_ctx.pv_line.get(search_ctx.ply) == Some(&mov) {
        return Searcher::INF;
    }
    if search_ctx.hash_move == Some(mov) {
        return Searcher::INF - 1;
    }

    let flags = mov.get_flags();

    // short-cut promotions
    if flags.promotion != Piece::None {
        let promoted_value = Board::PIECE_VALUES[flags.promotion as usize];
        return match flags.promotion {
            Piece::Queen | Piece::Knight => MoveBuckets::CAPTURES_PROMOTIONS + promoted_value,
            Piece::Bishop | Piece::Rook => MoveBuckets::UNDER_PROMOTIONS + promoted_value,
            _ => unreachable!(),
        };
    } else {
        match flags.move_type {
            MoveType::Capture => {
                let from = mov.get_from() as usize;
                let to = mov.get_to() as usize;
                let (victim, _) = search_ctx.board.pieces[to];
                let (attacker, _) = search_ctx.board.pieces[from];

                MVV_LVA[victim as usize][attacker as usize]
            }

            MoveType::EnPassantCapture => {
                MoveBuckets::CAPTURES_PROMOTIONS
                    + MVV_LVA[Piece::Pawn as usize][Piece::Pawn as usize]
            }

            _ => {
                let killers = &search_ctx.killers[search_ctx.ply];

                if Some(mov) == killers[0] {
                    MoveBuckets::KILLERS + 1 // give a small advantage
                } else if Some(mov) == killers[1] {
                    MoveBuckets::KILLERS
                } else {
                    search_ctx.history_heuristic.get(
                        mov.get_from(),
                        mov.get_to(),
                        search_ctx.board.side_to_move,
                    )
                }
            }
        }
    }
}

pub fn score(move_list: &MoveList, search_ctx: &SearchContext) -> ScoredMoveList {
    move_list
        .iter()
        .map(|&mov| {
            let score = score_move(mov, search_ctx);
            (mov, score)
        })
        .collect()
}

pub struct HistoryHeuristics {
    table: [[[AtomicI16; BOARD_SIZE]; BOARD_SIZE]; 2],
}

impl HistoryHeuristics {
    const HISTORY_MAX: i32 = 20_000;

    pub fn get(&self, from: Square, to: Square, color: Color) -> i16 {
        self.table[color as usize][from as usize][to as usize].load(Ordering::Relaxed)
    }

    // gravity formula
    pub fn update(&self, color: Color, from: Square, to: Square, bonus: i32) {
        let clamped_bonus = bonus.clamp(-Self::HISTORY_MAX, Self::HISTORY_MAX) as i32;

        self.table[color as usize][from as usize][to as usize]
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                let new = clamped_bonus - (old as i32) * clamped_bonus.abs() / Self::HISTORY_MAX;
                Some(new as i16)
            })
            .ok();
    }

    pub fn new() -> Self {
        Self {
            table: array::from_fn(|_| array::from_fn(|_| array::from_fn(|_| AtomicI16::new(0)))),
        }
    }
}

pub struct ScoredMoveIter<'a> {
    scored: &'a mut ScoredMoveList,
    index: usize,
}

impl<'a> Iterator for ScoredMoveIter<'a> {
    type Item = Move;

    // selection iteration is intentional; sorting would waste cycles on early cutoffs.
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.scored.len() {
            return None;
        }

        // Pick the best remaining move
        let mut best_index = self.index;
        for i in (self.index + 1)..self.scored.len() {
            if self.scored[i].1 > self.scored[best_index].1 {
                best_index = i;
            }
        }

        self.scored.swap(self.index, best_index);
        let (mov, _) = self.scored[self.index];
        self.index += 1;
        Some(mov)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.scored.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for ScoredMoveIter<'a> {}

pub trait ScoredIter {
    fn scored_iter(&mut self) -> ScoredMoveIter<'_>;
}

impl ScoredIter for ScoredMoveList {
    fn scored_iter(&mut self) -> ScoredMoveIter<'_> {
        ScoredMoveIter {
            scored: self,
            index: 0,
        }
    }
}
