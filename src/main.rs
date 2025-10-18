mod chess;

use chess::{attacks, board};
use std::io::{self, BufRead};

use crate::chess::{
    board::{BOARD_WIDTH, Board, Piece, Square, square_from_uci},
    moves::{Move, MoveFlag, MoveType},
};

/* ---------- perft helpers (unchanged) ---------- */
fn divide(board: &mut board::Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut nodes = 0;
    for mov in attacks::movegen::gen_color_moves(board) {
        let undo = board.make_move(mov);
        if attacks::movegen::is_legal_move(mov, board) {
            let subtree_nodes = perft(board, depth - 1);
            nodes += subtree_nodes;
            println!("{}: {}", mov.to_uci(), subtree_nodes);
        }
        board.undo_move(&undo);
    }
    nodes
}

fn perft(board: &mut board::Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut nodes = 0;
    for mov in attacks::movegen::gen_color_moves(board) {
        let undo = board.make_move(mov);
        if attacks::movegen::is_legal_move(mov, board) {
            nodes += perft(board, depth - 1);
        }
        board.undo_move(&undo);
    }
    nodes
}

/* ---------- UCI loop ---------- */
fn get_move_type(piece: Piece, to: Square, from: Square, board: &Board) -> MoveType {
    let lands_in_piece = board.pieces[to as usize].0 != Piece::None;

    if piece == Piece::Pawn {
        if Some(to) == board.en_passant_square && !lands_in_piece {
            return MoveType::EnPassantCapture;
        }
        if to.abs_diff(from) == (BOARD_WIDTH * 2) as u8 {
            return MoveType::DoublePawnPush;
        }
    } else if piece == Piece::King {
        let diff = to as i8 - from as i8;

        if diff == 2 {
            return MoveType::KingSideCastle;
        } else if diff == -2 {
            return MoveType::QueenSideCastle;
        }
    }

    if lands_in_piece {
        return MoveType::Capture;
    }

    MoveType::Quiet
}

fn main() -> Result<(), &'static str> {
    let stdin = io::stdin();
    let mut board = board::Board::new("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")?; // initial position

    for line in stdin.lock().lines().map_while(Result::ok) {
        let cmd: Vec<&str> = line.split_whitespace().collect();
        match cmd.first().copied() {
            Some("uci") => {
                println!("id name Sand");
                println!("id author P1x3r");
                println!("uciok");
            }
            Some("isready") => println!("readyok"),
            Some("quit") => std::process::exit(0),
            Some("position") => {
                // parse: position [fen <fen> | startpos ]  moves  m1 m2 ...
                let mut fen_given = false;
                let mut moves_start = None;
                for (i, &token) in cmd.iter().enumerate() {
                    if token == "fen" {
                        fen_given = true;
                        let fen = cmd[i + 1..i + 6].join(" ");
                        board = board::Board::new(&fen)?;
                        if i + 7 < cmd.len() && cmd[i + 7] == "moves" {
                            moves_start = Some(i + 8);
                        }
                        break;
                    } else if token == "startpos" {
                        board = board::Board::new(
                            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                        )?;
                    } else if token == "moves" {
                        moves_start = Some(i + 1);
                        break;
                    }
                }
                if let Some(start) = moves_start {
                    for mov_str in &cmd[start..] {
                        if mov_str.len() < 4 {
                            continue;
                        }

                        let from: Square = square_from_uci(&mov_str[0..2])?;
                        let to: Square = square_from_uci(&mov_str[2..4])?;
                        let promotion: Piece = if mov_str.len() > 4 {
                            match mov_str.chars().nth(4) {
                                Some('k') => Piece::Knight,
                                Some('b') => Piece::Bishop,
                                Some('r') => Piece::Rook,
                                Some('q') => Piece::Queen,
                                _ => Piece::None,
                            }
                        } else {
                            Piece::None
                        };
                        let (piece, _) = board.pieces[from as usize];

                        let _ = board.make_move(Move::new(
                            from,
                            to,
                            MoveFlag {
                                move_type: get_move_type(piece, to, from, &board),
                                promotion: promotion,
                            },
                        ));
                    }
                }
            }
            Some("go") => {
                if cmd.len() >= 3
                    && cmd[1] == "perft"
                    && let Ok(depth) = cmd[2].parse::<u32>()
                {
                    let nodes = divide(&mut board, depth);
                    println!("\nNodes searched: {}", nodes);
                }
            }
            _ => {} // ignore unknown commands
        }
    }
    Ok(())
}
