//! The interface every candidate law implements. The physics lab and the experiments drive a law
//! only through it, so the candidates can be compared on the same matter, side by side.
//!
//! A candidate owns its [`Matter`] plus whatever extra state its universe remembers. `step` is one
//! whole tick; it must be a pure function of the state (no clock, no randomness, no iteration
//! order that depends on hashing), so two peers that start equal stay equal. `digest` proves it.

use crate::material::Fnv64;
use crate::matter::Matter;

/// Named quantities a law reports after a tick. A conserved quantity must stay equal from tick to
/// tick; the experiments assert that.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Ledger {
    /// `(name, value)` in a fixed order.
    pub entries: Vec<(&'static str, i64)>,
}

impl Ledger {
    /// A ledger with one entry.
    pub fn with(mut self, name: &'static str, value: i64) -> Self {
        self.entries.push((name, value));
        self
    }

    /// The value of an entry.
    pub fn get(&self, name: &str) -> Option<i64> {
        self.entries.iter().find(|(n, _)| *n == name).map(|&(_, v)| v)
    }
}

/// One candidate universe running on a box of matter.
pub trait Candidate {
    /// Stable id (`"demon"`, `"belousov"`, ...).
    fn id(&self) -> &'static str;

    /// The matter the law runs on (it may change it: chemistry does).
    fn matter(&self) -> &Matter;

    /// Advance one whole tick.
    fn step(&mut self);

    /// Undo one whole tick exactly, when the law is reversible: `step` then `unstep` restores
    /// every byte. Returns false (and changes nothing) for a law that cannot run backwards.
    fn unstep(&mut self) -> bool {
        false
    }

    /// What a player's touch does at cell `i` with a signed strength: inject energy, excite,
    /// displace, emit a particle, as the law understands it. Laws without a notion ignore it.
    fn poke(&mut self, i: usize, strength: i32);

    /// How lively cell `i` is now, 0 (at rest) ..= 255, for a heat map.
    fn show(&self, i: usize) -> u8;

    /// The law's books after the last tick.
    fn ledger(&self) -> Ledger;

    /// Cells that did work in the last tick (the sparsity a world-scale host could exploit).
    fn active(&self) -> usize;

    /// Bytes of extra state per cell the law remembers beyond the material index.
    fn state_bytes_per_cell(&self) -> usize;

    /// Feed every byte of state that determines the future into `h`.
    fn hash_state(&self, h: &mut Fnv64);

    /// A digest of the whole state: equal digests on two peers mean equal futures.
    fn digest(&self) -> u64 {
        let mut h = Fnv64::new();
        for &m in self.matter().cells() {
            h.bytes(&m.to_le_bytes());
        }
        self.hash_state(&mut h);
        h.finish()
    }
}

/// How the lab finds a candidate: its identity and a constructor over a box of matter.
#[derive(Clone, Copy)]
pub struct Entry {
    /// Stable id, as [`Candidate::id`].
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// One sentence: what the universe remembers and the one rule.
    pub summary: &'static str,
    /// Build the candidate on a box of matter, at rest.
    pub build: fn(Matter) -> Box<dyn Candidate>,
}
