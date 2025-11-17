mod utils;

use sand::chess::*;
use sand::engine::transposition::{Bound, TT};

const MATE_SCORE: i16 = 30_000;
const INF: i16 = 32_000;

fn alpha_beta(
    board: &mut Board,
    mut alpha: i16,
    beta: i16,
    depth: usize,
    ply: usize,
    age: u8,
    tt: Option<&TT>,
) -> i16 {
    debug_assert_eq!(board.zobrist, board.calculate_zobrist());

    if depth == 0 {
        return match board.side_to_move {
            Color::White => board.evaluate(),
            Color::Black => -board.evaluate(),
        };
    }

    if let Some(tt) = tt {
        let entry = tt.probe(board.zobrist, depth);
        if let Some(e) = entry
            && e.depth == depth as u8
            && let Some(score) = e.probe(alpha, beta, ply)
        {
            return score;
        }
    }

    let mut best_score = -INF;
    let mut best_move = Move(0);
    let mut found_legal_move = false;

    for mov in gen_color_moves(board) {
        let undo = board.make_move(mov);
        if is_legal_move(mov, board) {
            found_legal_move = true;
            let score = -alpha_beta(board, -beta, -alpha, depth - 1, ply + 1, age, tt);

            if score > best_score {
                best_score = score;
                best_move = mov;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                board.undo_move(&undo);
                break;
            }
        }
        board.undo_move(&undo);
    }

    if found_legal_move {
        if let Some(tt) = tt {
            tt.store(
                board.zobrist,
                depth,
                best_score,
                best_move,
                Bound::from_score(best_score, alpha, beta),
                age,
                ply,
            );
        }
        best_score
    } else {
        let in_check = is_king_attcked(board.side_to_move, board);
        if in_check {
            -(MATE_SCORE - ply as i16)
        } else {
            0
        }
    }
}

#[test]
fn test_transposition() -> Result<(), &'static str> {
    const SEARCH_DEPTH: usize = 5;
    const TT_SIZE_MB: usize = 16;

    let tt = TT::new(TT_SIZE_MB);
    let mut age = 1;

    for line in utils::LARGE_TEST_EPDS {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }

        let fen = fields[..6].join(" ");
        let mut board = Board::new(&fen)?;

        for depth in 0..SEARCH_DEPTH {
            let score_with_tt = alpha_beta(&mut board, -INF, INF, depth, 0, age, Some(&tt));
            let score_without_tt = alpha_beta(&mut board, -INF, INF, depth, 0, age, None);

            if score_with_tt != score_without_tt {
                eprintln!("FEN: {}", fen);
                eprintln!("with TT: {}", score_with_tt);
                eprintln!("no TT:  {}", score_without_tt);
                panic!("TT mismatch");
            }
        }

        age += 1;
    }

    Ok(())
}
