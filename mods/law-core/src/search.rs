//! Deterministic helpers that find configurations with wanted readings, so experiments and the lab
//! never hard-code element bytes. The candidate sequence is a fixed SplitMix64 stream over a seed:
//! this only orders a search, it never decides a law's outcome.

use crate::material::{fit_raw, Block, Element, CAPACITY};
use crate::matter::bond_of;

/// A fixed stream of 64-bit values (SplitMix64).
#[derive(Clone, Debug)]
pub struct Stream(u64);

impl Stream {
    /// A stream from a seed.
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next value.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The next element.
    pub fn element(&mut self) -> Element {
        Element(self.next_u64().to_le_bytes()[..4].try_into().expect("four bytes"))
    }

    /// A value in `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

/// `count` occurrences of one element.
pub fn pure(e: Element, count: usize) -> Block {
    mix(&[(e, count)])
}

/// A configuration from `(element, count)` parts (at most [`CAPACITY`] occurrences in total).
pub fn mix(parts: &[(Element, usize)]) -> Block {
    let mut v = Vec::new();
    for &(e, n) in parts {
        v.extend(std::iter::repeat_n(e, n));
    }
    assert!(v.len() <= CAPACITY, "a configuration holds at most {CAPACITY} occurrences");
    let mut b = Block::new(&v).expect("within capacity");
    b.canonicalize();
    b
}

/// Whether every internal pair of distinct elements fits non-negatively (nothing wants to leave).
pub fn is_cohesive(b: &Block) -> bool {
    let e = b.elements();
    e.iter().enumerate().all(|(i, &x)| e[i + 1..].iter().all(|&y| x == y || fit_raw(x, y) >= 0))
}

/// The first configuration of the stream (one to `max_kinds` distinct elements, `size` occurrences
/// in total) that satisfies `want`. Panics after `tries` candidates: a search that fails is a bug
/// in the experiment, not something to paper over.
pub fn find(seed: u64, max_kinds: usize, size: usize, tries: usize, want: impl Fn(&Block) -> bool) -> Block {
    let mut s = Stream::new(seed);
    for _ in 0..tries {
        let kinds = 1 + s.below(max_kinds as u64) as usize;
        let mut parts: Vec<(Element, usize)> = (0..kinds).map(|_| (s.element(), 0)).collect();
        for k in 0..size {
            let at = if k < kinds { k } else { s.below(kinds as u64) as usize };
            parts[at].1 += 1;
        }
        let b = mix(&parts);
        if want(&b) {
            return b;
        }
    }
    panic!("no configuration in {tries} candidates of seed {seed} satisfies the search");
}

/// The first element of the stream whose single-element configuration of `size` satisfies `want`
/// together with the others already chosen: handy for building a family of related materials.
pub fn find_with(seed: u64, size: usize, tries: usize, want: impl Fn(&Block) -> bool) -> Block {
    let mut s = Stream::new(seed);
    for _ in 0..tries {
        let b = pure(s.element(), size);
        if want(&b) {
            return b;
        }
    }
    panic!("no pure configuration in {tries} candidates of seed {seed} satisfies the search");
}

/// Mean fit across a face between two configurations (Q8 of `QUANTUM`), re-exported for searches.
pub fn bond(a: &Block, b: &Block) -> i32 {
    bond_of(a, b)
}
