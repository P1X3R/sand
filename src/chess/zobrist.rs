use crate::chess::board::{BOARD_SIZE, BOARD_WIDTH, PIECE_TYPES};
use rand::{Rng, SeedableRng};
use std::array::from_fn;
use std::sync::LazyLock;

pub static ZOBRIST_PIECE: LazyLock<[[[u64; BOARD_SIZE]; PIECE_TYPES.len()]; 2]> =
    LazyLock::new(|| {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(1);
        from_fn(|_| from_fn(|_| from_fn(|_| rng.random())))
    });

pub static ZOBRIST_SIDE: LazyLock<u64> = LazyLock::new(|| {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(2);
    rng.random()
});

pub static ZOBRIST_CASTLING: LazyLock<[u64; 16]> = LazyLock::new(|| {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(3);
    from_fn(|_| rng.random())
});

pub static ZOBRIST_EN_PASSANT: LazyLock<[u64; BOARD_WIDTH]> = LazyLock::new(|| {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(4);
    from_fn(|_| rng.random())
});
