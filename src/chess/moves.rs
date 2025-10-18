use crate::chess::board::*;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MoveType {
    Quiet,
    DoublePawnPush,
    KingSideCastle,
    QueenSideCastle,
    Capture,
    EnPassantCapture,
    Invalid,
}

#[derive(Clone, Copy)]
pub struct MoveFlag {
    pub move_type: MoveType,
    pub promotion: Piece,
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
#[repr(transparent)]
pub struct Move(pub u16);

impl Move {
    #[inline(always)]
    pub fn new(from: Square, to: Square, move_flags: MoveFlag) -> Self {
        debug_assert!(move_flags.move_type != MoveType::Invalid);
        debug_assert!(from < BOARD_SIZE as u8 && to < BOARD_SIZE as u8);

        // --- bit-field layout of the final u16 ---
        // 15..12 : 4-bit flags  (move type + promotion)
        // 11..6  : 6-bit destination square
        // 5..0   : 6-bit origin square

        // 1. destination square → bits 11..6
        let to_encoded = (to as u16) << 6;

        // 2. promotion piece → upper half of the 4-bit flags
        //    * if no promotion → 0000
        //    * else            → 1ppp   (ppp = promotion-1 so Knight=0, Bishop=1, Rook=2, Queen=3)
        let promotion_bits = if move_flags.promotion != Piece::None {
            0b1000 | ((move_flags.promotion as u16) - 1) // 0b1000 .. 0b1011
        } else {
            0
        };

        // 3. move type → lower half of the 4-bit flags (bits 3..0 before the final shift)
        //    promotion_bits occupy bits 3..0 as well, but never clash because
        //    promotion_bits has bit-3 always set (0b1xxx) while pure move types
        //    never exceed 0b0111 (EnPassantCapture = 5).
        let move_flags_encoded = ((move_flags.move_type as u16) | promotion_bits) << 12;

        // 4. pack everything together
        Move(from as u16 | to_encoded | move_flags_encoded)
    }

    #[inline(always)]
    pub fn get_from(self) -> Square {
        (self.0 & 0x3f) as Square
    }

    #[inline(always)]
    pub fn get_to(self) -> Square {
        (self.0 >> 6 & 0x3f) as Square
    }

    #[inline(always)]
    pub fn get_flags(self) -> MoveFlag {
        let encoded_flags = (self.0 >> 12 & 0xf) as usize;
        let move_flag = crate::chess::attacks::tables::FLAGS_LUT[encoded_flags];

        debug_assert!(move_flag.move_type != MoveType::Invalid);
        move_flag
    }

    pub fn to_uci(self) -> String {
        let from_square = self.get_from();
        let to_square = self.get_to();

        let from_rank = from_square / BOARD_WIDTH as u8;
        let from_file = from_square % BOARD_WIDTH as u8;

        let to_rank = to_square / BOARD_WIDTH as u8;
        let to_file = to_square % BOARD_WIDTH as u8;

        let move_flags = self.get_flags();

        if move_flags.promotion != Piece::None {
            format!(
                "{}{}{}{}{}",
                (b'a' + from_file) as char,
                from_rank + 1,
                (b'a' + to_file) as char,
                to_rank + 1,
                move_flags.promotion.to_char()
            )
        } else {
            format!(
                "{}{}{}{}",
                (b'a' + from_file) as char,
                from_rank + 1,
                (b'a' + to_file) as char,
                to_rank + 1
            )
        }
    }
}
