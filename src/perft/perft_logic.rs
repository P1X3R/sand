use sand::chess::*;

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct PerftTTEntry {
    zobrist: u64,
    nodes: u32,
    depth: u8,
    padding: [u8; 3], // force 16-byte size
}

pub struct PerftTT {
    table: Box<[PerftTTEntry]>,
    mask: usize,
}

impl PerftTT {
    #[inline]
    pub fn new(megabytes: usize) -> Self {
        const MIB: usize = 1 << 20;
        let entry_size = std::mem::size_of::<PerftTTEntry>();
        let requested_bytes = megabytes * MIB;

        let mut entries = requested_bytes / entry_size;
        entries = entries.next_power_of_two();

        let table = vec![
            PerftTTEntry {
                zobrist: 0,
                nodes: 0,
                depth: 0,
                padding: [0; 3],
            };
            entries
        ]
        .into_boxed_slice();

        Self {
            table,
            mask: entries - 1,
        }
    }

    #[inline]
    fn index(&self, zobrist: u64) -> usize {
        (zobrist as usize) & self.mask
    }

    #[inline]
    pub fn probe(&self, zobrist: u64, depth: u8) -> Option<u32> {
        let e = unsafe { self.table.get_unchecked(self.index(zobrist)) };
        if e.zobrist == zobrist && e.depth == depth {
            Some(e.nodes)
        } else {
            None
        }
    }

    #[inline]
    pub fn store(&mut self, zobrist: u64, depth: u8, nodes: u32) {
        let idx = self.index(zobrist);
        let e = unsafe { self.table.get_unchecked_mut(idx) };

        // Minimal replacement policy
        if depth >= e.depth {
            e.zobrist = zobrist;
            e.nodes = nodes;
            e.depth = depth;
        }
    }
}

pub fn perft(board: &mut Board, depth: u8, tt: &mut PerftTT) -> u32 {
    debug_assert_eq!(board.zobrist, board.calculate_zobrist());

    if depth == 0 {
        return 1;
    }
    let zobrist = board.zobrist;
    if let Some(nodes) = tt.probe(zobrist, depth) {
        return nodes;
    }

    let mut nodes = 0;

    for mov in gen_color_moves(board) {
        let undo = board.make_move(mov);
        if is_legal_move(mov, board) {
            nodes += perft(board, depth - 1, tt);
        }
        board.undo_move(&undo);
    }

    tt.store(zobrist, depth, nodes);
    nodes
}

pub fn _divide(board: &mut Board, depth: u8, tt: &mut PerftTT) -> u32 {
    if depth == 0 {
        return 1;
    }

    let mut nodes = 0;

    for mov in gen_color_moves(board) {
        let undo = board.make_move(mov);
        if is_legal_move(mov, board) {
            debug_assert_eq!(board.zobrist, board.calculate_zobrist());
            let subtree_nodes = perft(board, depth - 1, tt);
            nodes += subtree_nodes;
            println!("{}: {}", mov.to_uci(), subtree_nodes);
        }
        board.undo_move(&undo);
    }

    nodes
}
