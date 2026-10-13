//! Candidate law `demon`: the microcanonical law (Landau, Zeldovich, Creutz). Selective transfer's
//! chemistry keeps its moves, but the universe keeps an energy ledger. Every matter cell holds
//! `heat`, an unsigned count of quanta, and the total
//!
//! `E = Σ heat − Σ binding − work`, with `binding(c) = floor(internal_fit(c) / UNIT)`,
//!
//! is conserved exactly by every tick (`work` is what pokes put in or took out). One tick is:
//!
//! 1. **Conduction.** Every face between two matter cells carries the antisymmetric flux
//!    `trunc(max(bond, 0) · (q_a − q_b) / 2^13)`, computed from the old heat and applied at once.
//! 2. **Chemistry.** The tick's dimer phase pairs every cell with at most one face neighbour (axis
//!    `t mod 3`, parity `(t / 3) mod 2`). Each pair is a contact that makes **the best
//!    selective-transfer move each cell can pay for from its own heat**: a move (a transfer, or a
//!    swap at capacity) changes each cell's binding by `δ`, and each cell's heat by the same `δ`,
//!    so it needs `q + δ ≥ 0` in both cells, and a giving cell must keep at least one quantum
//!    (`q + δ ≥ 1`: no heat, no motion). Among the affordable moves the highest fit gain wins, with
//!    selective transfer's tie order. A cell left empty hands its heat to the other one.
//!
//! A move therefore leaves every cell's energy `q − binding` unchanged: chemistry only
//! rearranges, and energy travels only by conduction (and with matter that moves into void).
//! Pulling an occurrence out of its block costs the giving cell its holding: that is the
//! activation barrier, and the receiving cell is paid the occurrence's attraction to it.
//!
//! Cold cells never give and cold faces never conduct, so the warm cells are the whole work set:
//! a cold world costs nothing. Moves are evaluated once per ordered pair of materials and memoised.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use pwc_law_core::{Candidate, Entry, Ledger, Mat, Matter, VOID};
use pwc_mod_api::material;

use crate::material::{fit_raw, Block, Element, Fnv64, CAPACITY, QUANTUM};

/// One quantum of heat in raw fit units: 1/256 of `QUANTUM`, the scale every reading uses.
pub const UNIT: i64 = (QUANTUM >> 8) as i64;

/// Conductance shift: the smallest `s` with `6 · max_bond ≤ 2^s` (every bond is below 1152), so a
/// cell's six face fluxes never move more than its heat difference (a maximum principle).
pub const CONDUCT_SHIFT: u32 = 13;

/// Most heat a poke may leave in one cell, so heat stays far from `u32` overflow.
pub const HEAT_CAP: u32 = 1 << 28;

/// The law's entry for the physics lab.
pub const ENTRY: Entry = Entry {
    id: "demon",
    name: "Demon",
    summary: "Each cell holds heat; a contact makes the best selective-transfer move each cell can \
              pay for from its own heat, and Σheat − Σbinding is conserved exactly.",
    build: |m| Box::new(Demon::new(m)),
};

/// The floor of `fit / UNIT`: a configuration's binding in quanta.
#[inline]
pub fn binding_of(internal: i64) -> i64 {
    internal.div_euclid(UNIT)
}

/// What a move does to the contact's configurations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// One occurrence of `e` leaves side `from` (0 = lower cell, 1 = upper cell).
    Transfer { from: u8, e: Element },
    /// `e` leaves the lower cell and `f` the upper one, at once.
    Swap { e: Element, f: Element },
}

/// One move of a contact, in the memo.
#[derive(Clone, Copy, Debug)]
pub struct Move {
    /// Least heat each cell (lower, upper) must hold: `max(0, −δ)`, plus one for a giving cell.
    pub need: [u32; 2],
    /// Binding change of each cell in quanta (positive releases heat into that cell).
    pub delta: [i32; 2],
    kind: Kind,
    /// The two materials after the move, once it has run (interned lazily).
    result: Option<(Mat, Mat)>,
}

impl Move {
    /// Whether cells holding `q` can pay for it.
    #[inline]
    pub fn affordable(&self, q: [u32; 2]) -> bool {
        q[0] >= self.need[0] && q[1] >= self.need[1]
    }
}

/// A move before pruning, with its rank.
struct Ranked {
    gain: i64,
    key: (u8, u8, Element, Element),
    mv: Move,
}

/// A move from the two cells' fit before and after, and which cells give.
fn priced(before: [i64; 2], after: [i64; 2], gives: [bool; 2], kind: Kind) -> Move {
    let mut need = [0u32; 2];
    let mut delta = [0i32; 2];
    for s in 0..2 {
        let d = binding_of(after[s]) - binding_of(before[s]);
        delta[s] = i32::try_from(d).expect("a binding change fits i32");
        need[s] = u32::try_from((-d).max(0) + i64::from(gives[s])).expect("a need fits u32");
    }
    Move { need, delta, kind, result: None }
}

/// Every move of the ordered contact `(a, b)` the greedy rule can ever choose: ranked by fit gain
/// (then selective transfer's tie order), dropping each move that a better one dominates (needs no
/// more heat in either cell).
fn contact_moves(a: &Block, b: &Block) -> Vec<Move> {
    let fit = [a.internal_fit(), b.internal_fit()];
    let pull = |e: Element, other: &Block| -> i64 { other.elements().iter().map(|&o| i64::from(fit_raw(e, o))).sum() };
    let mut opts: Vec<Ranked> = Vec::new();
    let sides = [a, b];
    for from in 0..2usize {
        let (src, dst) = (sides[from], sides[1 - from]);
        if dst.len() >= CAPACITY {
            continue;
        }
        for (i, &e) in src.elements().iter().enumerate() {
            if i > 0 && src.elements()[i - 1] == e {
                continue;
            }
            let h = i64::from(src.holding()[i]);
            let p = pull(e, dst);
            let mut after = fit;
            after[from] -= h;
            after[1 - from] += p;
            let mut gives = [false; 2];
            gives[from] = true;
            let kind = Kind::Transfer { from: from as u8, e };
            opts.push(Ranked { gain: p - h, key: (0, from as u8, e, Element::default()), mv: priced(fit, after, gives, kind) });
        }
    }
    if a.len() == CAPACITY || b.len() == CAPACITY {
        for (i, &e) in a.elements().iter().enumerate() {
            if i > 0 && a.elements()[i - 1] == e {
                continue;
            }
            let (he, pe) = (i64::from(a.holding()[i]), pull(e, b));
            for (j, &f) in b.elements().iter().enumerate() {
                if j > 0 && b.elements()[j - 1] == f {
                    continue;
                }
                let (hf, pf) = (i64::from(b.holding()[j]), pull(f, a));
                let ef = i64::from(fit_raw(e, f));
                let after = [fit[0] - he + pf - ef, fit[1] - hf + pe - ef];
                opts.push(Ranked {
                    gain: pe - he + pf - hf - 2 * ef,
                    key: (1, 0, e, f),
                    mv: priced(fit, after, [true, true], Kind::Swap { e, f }),
                });
            }
        }
    }
    opts.sort_by(|x, y| y.gain.cmp(&x.gain).then(x.key.cmp(&y.key)));
    let mut out: Vec<Move> = Vec::new();
    for o in opts {
        if !out.iter().any(|k| k.need[0] <= o.mv.need[0] && k.need[1] <= o.mv.need[1]) {
            out.push(o.mv);
        }
    }
    out
}

/// The two configurations after a move.
fn apply(a: &Block, b: &Block, kind: Kind) -> (Block, Block) {
    let mut va = a.elements().to_vec();
    let mut vb = b.elements().to_vec();
    let take = |v: &mut Vec<Element>, e: Element| {
        let at = v.iter().position(|&x| x == e).expect("the moved occurrence is there");
        v.remove(at);
    };
    match kind {
        Kind::Transfer { from: 0, e } => {
            take(&mut va, e);
            vb.push(e);
        }
        Kind::Transfer { e, .. } => {
            take(&mut vb, e);
            va.push(e);
        }
        Kind::Swap { e, f } => {
            take(&mut va, e);
            take(&mut vb, f);
            va.push(f);
            vb.push(e);
        }
    }
    let block = |v: &[Element]| {
        let mut b = Block::new(v).expect("within capacity");
        b.canonicalize();
        b
    };
    (block(&va), block(&vb))
}

/// A multiplicative hasher for the memo's pair keys (its iteration order is never observed).
#[derive(Default)]
struct PairHasher(u64);

impl Hasher for PairHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_u32(&mut self, k: u32) {
        self.0 = u64::from(k).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

type Memo = HashMap<u32, (u32, u32), BuildHasherDefault<PairHasher>>;

/// Flag bits of the per-tick bookkeeping (scratch, never state).
const LISTED: u8 = 1;
const WARM: u8 = 2;
const HOT: u8 = 4;

/// The demon universe.
pub struct Demon {
    matter: Matter,
    heat: Vec<u32>,
    tick: u64,
    work: i64,
    /// Binding (quanta) of each palette entry.
    binding: Vec<i64>,
    /// Cells holding heat: the work set (sorted at the start of a tick).
    warm: Vec<u32>,
    /// Scratch: cells touched this tick, their flags and their conduction sums.
    touched: Vec<u32>,
    flag: Vec<u8>,
    dq: Vec<i64>,
    memo: Memo,
    moves: Vec<Move>,
    active: usize,
    reactions: u64,
}

impl Demon {
    /// The law at rest (no heat) on a box of matter.
    pub fn new(matter: Matter) -> Self {
        let n = matter.grid.len();
        let binding = (0..matter.palette_len()).map(|m| binding_of(matter.reading(m as Mat).internal)).collect();
        Self {
            matter,
            heat: vec![0; n],
            tick: 0,
            work: 0,
            binding,
            warm: Vec::new(),
            touched: Vec::new(),
            flag: vec![0; n],
            dq: vec![0; n],
            memo: Memo::default(),
            moves: Vec::new(),
            active: 0,
            reactions: 0,
        }
    }

    /// Heat of cell `i` in quanta.
    #[inline]
    pub fn heat(&self, i: usize) -> u32 {
        self.heat[i]
    }

    /// Binding of a material in quanta.
    #[inline]
    pub fn binding(&self, m: Mat) -> i64 {
        self.binding[m as usize]
    }

    /// Moves made since the start.
    pub fn reactions(&self) -> u64 {
        self.reactions
    }

    /// External work put in by pokes (negative when pokes took heat out).
    pub fn work(&self) -> i64 {
        self.work
    }

    /// Σ heat, Σ binding and the conserved `E = Σ heat − Σ binding − work`, recounted from the cells.
    pub fn energy(&self) -> (i64, i64, i64) {
        let heat: i64 = self.heat.iter().map(|&q| i64::from(q)).sum();
        let bind: i64 = self.matter.cells().iter().map(|&m| self.binding[m as usize]).sum();
        (heat, bind, heat - bind - self.work)
    }

    /// The moves the greedy rule can choose in the ordered contact `(a, b)`, best first.
    pub fn moves(&mut self, a: Mat, b: Mat) -> Vec<Move> {
        let (s, l) = self.moves_of(a, b);
        self.moves[s as usize..(s + l) as usize].to_vec()
    }

    /// What the ordered contact `(a, b)` does when its cells hold `q`: the materials after its move
    /// and the move, or `None` when it cannot move.
    pub fn outcome(&mut self, a: Mat, b: Mat, q: [u32; 2]) -> Option<(Mat, Mat, Move)> {
        let (s, l) = self.moves_of(a, b);
        let k = (s..s + l).find(|&k| self.moves[k as usize].affordable(q))? as usize;
        let (na, nb) = self.result(k, a, b);
        Some((na, nb, self.moves[k]))
    }

    /// The memoised moves of the ordered contact `(a, b)`: start and length in `moves`.
    fn moves_of(&mut self, a: Mat, b: Mat) -> (u32, u32) {
        let key = (u32::from(a) << 16) | u32::from(b);
        if let Some(&r) = self.memo.get(&key) {
            return r;
        }
        let list = if a == VOID && b == VOID {
            Vec::new()
        } else {
            contact_moves(self.matter.block(a), self.matter.block(b))
        };
        let r = (self.moves.len() as u32, list.len() as u32);
        self.moves.extend(list);
        self.memo.insert(key, r);
        r
    }

    /// The materials after memo move `k` of contact `(a, b)`, interning them the first time.
    fn result(&mut self, k: usize, a: Mat, b: Mat) -> (Mat, Mat) {
        if let Some(r) = self.moves[k].result {
            return r;
        }
        let (ba, bb) = apply(self.matter.block(a), self.matter.block(b), self.moves[k].kind);
        let r = (self.intern(&ba), self.intern(&bb));
        self.moves[k].result = Some(r);
        r
    }

    fn intern(&mut self, b: &Block) -> Mat {
        let m = self.matter.intern(b);
        while self.binding.len() < self.matter.palette_len() {
            let k = self.binding.len() as Mat;
            self.binding.push(binding_of(self.matter.reading(k).internal));
        }
        m
    }

    /// Run the contact of cells `lo < hi`: its best affordable move, if any.
    fn react(&mut self, lo: usize, hi: usize) {
        let q = [self.heat[lo], self.heat[hi]];
        let (a, b) = (self.matter.get(lo), self.matter.get(hi));
        let (s, l) = self.moves_of(a, b);
        let Some(k) = (s..s + l).find(|&k| self.moves[k as usize].affordable(q)) else { return };
        let k = k as usize;
        let (na, nb) = self.result(k, a, b);
        let d = self.moves[k].delta;
        let mut qa = (i64::from(q[0]) + i64::from(d[0])) as u32;
        let mut qb = (i64::from(q[1]) + i64::from(d[1])) as u32;
        if na == VOID {
            qb += qa; // heat lives in matter: it leaves with the last occurrence
            qa = 0;
        } else if nb == VOID {
            qa += qb;
            qb = 0;
        }
        self.matter.set(lo, na);
        self.matter.set(hi, nb);
        self.heat[lo] = qa;
        self.heat[hi] = qb;
        self.reactions += 1;
    }

    #[inline]
    fn touch(&mut self, i: usize) {
        if self.flag[i] & LISTED == 0 {
            self.flag[i] |= LISTED;
            self.touched.push(i as u32);
        }
    }
}

impl Candidate for Demon {
    fn id(&self) -> &'static str {
        "demon"
    }

    fn matter(&self) -> &Matter {
        &self.matter
    }

    fn step(&mut self) {
        let t = self.tick;
        self.tick += 1;
        if self.warm.is_empty() {
            self.active = 0;
            return;
        }
        self.warm.sort_unstable();
        self.warm.dedup();
        let g = self.matter.grid;
        let dims = [g.nx, g.ny, g.nz];
        // 1. Conduction from the old heat. Each face once: by its warm end, the lower if both are.
        self.touched.clear();
        for &i in &self.warm {
            self.flag[i as usize] = LISTED | WARM;
        }
        self.touched.extend_from_slice(&self.warm);
        for w in 0..self.warm.len() {
            let i = self.warm[w] as usize;
            let mi = self.matter.get(i);
            let (x, y, z) = g.pos(i);
            let p = [x, y, z];
            for d in 0..6 {
                let ax = d % 3;
                let inside = if d < 3 { p[ax] > 0 } else { p[ax] + 1 < dims[ax] };
                if !inside {
                    continue;
                }
                let j = (i as isize + g.stride(d)) as usize;
                if j < i && self.flag[j] & WARM != 0 {
                    continue;
                }
                let mj = self.matter.get(j);
                let c = i64::from(self.matter.bond(mi, mj).max(0));
                if c == 0 {
                    continue; // void bonds 0: it never conducts
                }
                let f = c * (i64::from(self.heat[i]) - i64::from(self.heat[j])) / (1 << CONDUCT_SHIFT);
                if f != 0 {
                    self.dq[i] -= f;
                    self.dq[j] += f;
                    self.touch(j);
                }
            }
        }
        for k in 0..self.touched.len() {
            let i = self.touched[k] as usize;
            self.heat[i] = (i64::from(self.heat[i]) + self.dq[i]) as u32;
            self.dq[i] = 0;
            if self.heat[i] > 0 {
                self.flag[i] |= HOT;
            }
        }
        self.touched.sort_unstable();
        // 2. Chemistry on this tick's dimer phase, in cell order. Contacts are disjoint, so only
        //    the order in which new configurations are interned depends on it.
        let ax = (t % 3) as usize;
        let parity = ((t / 3) % 2) as u32;
        for k in 0..self.touched.len() {
            let i = self.touched[k] as usize;
            if self.flag[i] & HOT == 0 {
                continue;
            }
            let (x, y, z) = g.pos(i);
            let c = [x, y, z][ax];
            let (lo, hi) = if (c + parity) & 1 == 0 {
                if c + 1 >= dims[ax] {
                    continue;
                }
                (i, (i as isize + g.stride(ax + 3)) as usize)
            } else {
                if c == 0 {
                    continue;
                }
                let j = (i as isize + g.stride(ax)) as usize;
                if self.flag[j] & HOT != 0 {
                    continue; // the lower cell runs this contact
                }
                (j, i)
            };
            self.react(lo, hi);
            self.touch(lo);
            self.touch(hi);
        }
        // 3. The new work set: every touched cell still holding heat.
        self.active = self.touched.len();
        self.warm.clear();
        for k in 0..self.touched.len() {
            let i = self.touched[k] as usize;
            self.flag[i] = 0;
            if self.heat[i] > 0 {
                self.warm.push(i as u32);
            }
        }
    }

    fn poke(&mut self, i: usize, strength: i32) {
        if self.matter.get(i) == VOID {
            return; // heat lives in matter only
        }
        let q = self.heat[i];
        let next = if strength >= 0 {
            q.saturating_add(strength.unsigned_abs()).min(HEAT_CAP.max(q))
        } else {
            q.saturating_sub(strength.unsigned_abs())
        };
        self.work += i64::from(next) - i64::from(q);
        self.heat[i] = next;
        if q == 0 && next > 0 {
            self.warm.push(i as u32);
        }
    }

    fn show(&self, i: usize) -> u8 {
        let q = self.heat[i];
        if q == 0 { 0 } else { (8 * (32 - q.leading_zeros())).min(255) as u8 }
    }

    fn ledger(&self) -> Ledger {
        let (heat, bind, e) = self.energy();
        Ledger::default()
            .with("heat", heat)
            .with("binding", bind)
            .with("work", self.work)
            .with("energy", e)
            .with("occurrences", self.matter.occurrences())
            .with("reactions", self.reactions as i64)
    }

    fn active(&self) -> usize {
        self.active
    }

    fn state_bytes_per_cell(&self) -> usize {
        4
    }

    fn hash_state(&self, h: &mut Fnv64) {
        h.bytes(&(self.tick % 6).to_le_bytes());
        for &q in &self.heat {
            h.bytes(&q.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests;
