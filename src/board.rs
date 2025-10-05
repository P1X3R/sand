pub const BOARD_WIDTH: usize = 8;
pub const BOARD_SIZE: usize = 64;

#[derive(PartialEq)]
pub enum Color {
    White,
    Black,
}

pub const RANK_BB_1: u64 = 0xFF;
pub const RANK_BB_2: u64 = 0xFF00;
pub const RANK_BB_3: u64 = 0xFF0000;
pub const RANK_BB_4: u64 = 0xFF000000;
pub const RANK_BB_5: u64 = 0xFF00000000;
pub const RANK_BB_6: u64 = 0xFF0000000000;
pub const RANK_BB_7: u64 = 0xFF000000000000;
pub const RANK_BB_8: u64 = 0xFF00000000000000;

#[inline]
pub fn to_square(rank: isize, file: isize) -> isize {
    (rank * BOARD_WIDTH as isize) + file
}

#[inline]
pub fn valid_axis(axis: isize) -> bool {
    axis >= 0 && axis < BOARD_WIDTH as isize
}
