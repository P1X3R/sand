use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::{chess::*, engine::search::Searcher};

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Bound {
    Exact,
    Upper,
    Lower,
}

impl Default for Bound {
    fn default() -> Self {
        Bound::Exact
    }
}

impl Bound {
    pub fn from_score(score: i16, alpha: i16, beta: i16) -> Bound {
        if score >= beta {
            Bound::Lower // fail-high → lower bound
        } else if score <= alpha {
            Bound::Upper // fail-low → upper bound
        } else {
            Bound::Exact // score within alpha-beta window
        }
    }

    pub fn from_u64(n: u64) -> Bound {
        match n {
            0 => Bound::Exact,
            1 => Bound::Upper,
            2 => Bound::Lower,
            _ => Bound::Exact,
        }
    }
}

#[repr(C, align(16))]
#[derive(Default)]
pub(crate) struct TTEntry {
    key: AtomicU64, // zobrist
    data: AtomicU64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TTEntryData {
    pub depth: u8,
    score: i16,
    bound: Bound,
    pub best_move: Move,
}

impl TTEntryData {
    fn decode_mate(score: i16, ply: usize) -> i16 {
        if score > Searcher::CHECKMATE_THRESHOLD {
            score + ply as i16
        } else if score < -Searcher::CHECKMATE_THRESHOLD {
            score - ply as i16
        } else {
            score // not a checkmate
        }
    }

    pub fn probe(&self, alpha: &mut i16, beta: &mut i16, ply: usize) -> Option<i16> {
        let score = TTEntryData::decode_mate(self.score, ply);

        match self.bound {
            Bound::Exact => {
                // We know the exact minimax score here.
                return Some(score);
            }
            Bound::Lower => {
                // This is a lower bound: true score >= this.
                if score > *alpha {
                    *alpha = score;
                }
            }
            Bound::Upper => {
                // This is an upper bound: true score <= this.
                if score < *beta {
                    *beta = score;
                }
            }
        }

        if *alpha >= *beta {
            // Bound proved a cutoff.
            return Some(score);
        }

        None
    }
}

struct EntryEncoding;
impl EntryEncoding {
    const DEPTH_SHIFT: u64 = 56;
    const AGE_SHIFT: u64 = 48;
    const SCORE_SHIFT: u64 = 32;
    const BOUND_SHIFT: u64 = 16;
}

impl TTEntry {
    pub fn encode_mate(score: i16, ply: usize) -> i16 {
        if score > Searcher::CHECKMATE_THRESHOLD {
            score - ply as i16
        } else if score < -Searcher::CHECKMATE_THRESHOLD {
            score + ply as i16
        } else {
            score // not a checkmate
        }
    }

    pub fn get_depth(&self) -> usize {
        (self.data.load(Ordering::Acquire) >> EntryEncoding::DEPTH_SHIFT) as usize
    }
    pub fn get_age(&self) -> u8 {
        ((self.data.load(Ordering::Acquire) >> EntryEncoding::AGE_SHIFT) & 0xFF) as u8
    }

    pub fn store(&self, key: u64, depth: usize, score: i16, mov: Move, bound: Bound, age: u8) {
        // pack: [depth:8][age:8][score:16][bound:8][move:16] == 56 bits used
        let packed = (depth as u64) << EntryEncoding::DEPTH_SHIFT
            | (age as u64) << EntryEncoding::AGE_SHIFT
            | (score as u16 as u64) << EntryEncoding::SCORE_SHIFT
            | (bound as u64) << EntryEncoding::BOUND_SHIFT
            | (mov.0 as u64);

        self.key.store(key, Ordering::Release);
        self.data.store(packed, Ordering::Release);
    }

    pub fn load(&self) -> TTEntryData {
        let data = self.data.load(Ordering::Acquire);

        TTEntryData {
            depth: (data >> EntryEncoding::DEPTH_SHIFT) as u8,
            score: (data >> EntryEncoding::SCORE_SHIFT) as u16 as i16,
            bound: Bound::from_u64((data >> EntryEncoding::BOUND_SHIFT) & 0xFF),
            best_move: Move((data & 0xFFFF) as u16),
        }
    }

    pub fn get_key(&self) -> u64 {
        self.key.load(Ordering::Acquire)
    }
}

const BUCKET_SIZE: usize = 2;
type Bucket<const N: usize> = [TTEntry; N];

pub struct TT {
    table: Box<[Bucket<BUCKET_SIZE>]>,
    used: AtomicUsize,
    mask: usize,
}

impl TT {
    pub fn new(megabytes: usize) -> TT {
        const MIB: usize = 1 << 20;
        let entry_size = std::mem::size_of::<TTEntry>();
        let requested_bytes = megabytes * MIB;

        let mut entries = requested_bytes / entry_size;
        entries = entries.next_power_of_two();

        let table = (0..entries)
            .map(|_| Bucket::default())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        TT {
            table,
            used: AtomicUsize::new(0),
            mask: entries - 1,
        }
    }

    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    pub fn probe(&self, key: u64, depth: usize) -> Option<TTEntryData> {
        debug_assert!(depth < Searcher::MAX_PLY);

        for entry in &self.table[self.index(key)] {
            if entry.get_key() == key {
                let data = entry.load();
                if data.depth >= depth as u8 {
                    return Some(data);
                }
            }
        }

        None
    }

    pub fn store(
        &self,
        key: u64,
        depth: usize,
        score: i16,
        best_move: Move,
        bound: Bound,
        age: u8,
        ply: usize,
    ) {
        debug_assert!(depth < Searcher::MAX_PLY);
        debug_assert!(ply < Searcher::MAX_PLY);

        let bucket = &self.table[self.index(key)];
        let score = TTEntry::encode_mate(score, ply);

        // 1. replace same key if deeper
        for entry in bucket.iter() {
            if entry.get_key() == key {
                if depth >= entry.get_depth() {
                    entry.store(key, depth, score, best_move, bound, age);
                }
                return;
            }
        }

        // 2. replace old slot
        for entry in bucket.iter() {
            if entry.get_age() != age {
                entry.store(key, depth, score, best_move, bound, age);
                self.used.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }

        // 3. replace shallowest
        let mut min_idx = 0;
        let mut min_depth = bucket[0].get_depth();
        for (i, entry) in bucket.iter().enumerate().skip(1) {
            let depth = entry.get_depth();
            if depth < min_depth {
                min_depth = depth;
                min_idx = i;
            }
        }

        bucket[min_idx].store(key, depth, score, best_move, bound, age);
    }

    pub fn get_hashfull(&self) -> u16 {
        let total = (self.table.len() * BUCKET_SIZE) as u64;
        let used = (self.used.load(Ordering::Relaxed) as u64).min(total);
        ((used * 1000u64) / total) as u16
    }
}
