//! Candidate law `zeldovich`: chemistry in a vibrating world under one exactly conserved energy.
//!
//! Energy is a count of **quanta**. Each matter cell holds six counts, the quanta heading for each
//! of its faces. Binding is energy too: a configuration holds `floor(internal_fit / UNIT)` quanta
//! of it. The universe conserves, exactly and in integers,
//!
//! `E = Σ quanta − Σ binding − work`,
//!
//! where `work` is what pokes added. One tick is two rules, both local and both integer:
//!
//! 1. **Face rule (exchange).** The quanta heading for a face from both sides are its impact `H`.
//!    They may carry one occurrence across the face: among the moves that change the fit by more
//!    than selective transfer's threshold (in either direction), the one that law ranks first
//!    among those `H` can pay for (its barrier, the binding the occurrence holds in its block,
//!    and any binding the move loses). The impact plus the move's released binding is then split
//!    evenly and enters both cells through the face. A cell takes part in at most one move per
//!    tick; faces are claimed in canonical order (lower cell, then axis).
//! 2. **Cell rule (transport).** Every other quantum crosses the face it heads for when that face
//!    conducts (both sides matter, bond > 0), entering the neighbour; otherwise it is turned back
//!    and enters its own cell again through that face. A cell deals the quanta that entered
//!    through a face evenly over its other conducting faces, or sends them straight on when it
//!    has none. The remainder of an uneven deal goes round-robin from the cell's **rotor**
//!    (a rotor-router, Priezzhev's Eulerian walker), so dealing prefers no side.
//!
//! Dealing is doubly stochastic on average: every conducting face of a cell receives a share from
//! every other one, so a uniform flow per face is stationary and heat spreads to equal energy per
//! bond. Wires (two conducting faces) carry pulses straight and turn them at corners; bulk matter
//! scatters them, so heat diffuses.
//!
//! Nothing else exists: no temperature field, no time step, no randomness, no floating point.
//! A cell without quanta does nothing, so a cold world costs nothing.

use pwc_mod_api::material;

use std::collections::HashMap;

use crate::material::{Block, CAPACITY, Element, Fnv64, QUANTUM, fit_raw};
use pwc_law_core::grid::opposite;
use pwc_law_core::{Candidate, Entry, Ledger, Mat, Matter, VOID};

#[cfg(test)]
mod tests;

/// The energy quantum in raw fit units: `QUANTUM / 8`, the per-occurrence step of selective
/// transfer's threshold (`gain > N · QUANTUM / 8`), the smallest change of fit that law counts.
pub const UNIT: i64 = QUANTUM as i64 / 8;

/// Binding energy of an internal fit, in quanta (floor, so exact in integers).
#[inline]
pub fn binding(internal: i64) -> i64 {
    internal.div_euclid(UNIT)
}

/// One occurrence crossing a face.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    /// True when the occurrence goes from the lower cell to the upper one (`x → y`).
    pub up: bool,
    /// The element that crosses.
    pub element: Element,
    /// Binding it must be lifted out of: what its source loses when it leaves (≥ 0).
    pub barrier: i64,
    /// Binding gained by the whole move (negative: the move stores energy).
    pub release: i64,
    /// Raw fit gain of the move, selective transfer's ranking.
    pub gain: i64,
}

/// The face rule's choice: the move an impact of `impact` quanta makes between `x` (lower cell)
/// and `y` (upper cell), or `None`. A move is a chemical event only when it changes the fit by
/// more than selective transfer's threshold, in either direction (`|gain| > N · QUANTUM / 8` for
/// `N` occurrences in the contact). It is affordable when `impact ≥ barrier` and
/// `impact + release ≥ 0`. Among affordable events the highest raw gain wins, then `x → y`, then
/// the smaller element (selective transfer's order).
pub fn react(x: &Block, y: &Block, impact: i64) -> Option<Move> {
    let mut best = None;
    if y.len() < CAPACITY {
        scan(x, y, true, impact, &mut best);
    }
    if x.len() < CAPACITY {
        scan(y, x, false, impact, &mut best);
    }
    best
}

fn scan(src: &Block, dst: &Block, up: bool, impact: i64, best: &mut Option<Move>) {
    let threshold = (src.len() + dst.len()) as i64 * UNIT;
    let (is, id) = (src.internal_fit(), dst.internal_fit());
    let (bs, bd) = (binding(is), binding(id));
    let hold = src.holding();
    let mut prev = None;
    for (k, &e) in src.elements().iter().enumerate() {
        // Equal elements hold equally: one is enough.
        if prev == Some(e) {
            continue;
        }
        prev = Some(e);
        let rest = binding(is - i64::from(hold[k]));
        let barrier = (bs - rest).max(0);
        if barrier > impact {
            continue;
        }
        let pull: i64 = dst.elements().iter().map(|&f| i64::from(fit_raw(e, f))).sum();
        let gain = pull - i64::from(hold[k]);
        let release = rest + binding(id + pull) - bs - bd;
        if gain.abs() <= threshold || impact + release < 0 {
            continue;
        }
        let m = Move { up, element: e, barrier, release, gain };
        let wins = match best {
            None => true,
            Some(b) => (m.gain, m.up, std::cmp::Reverse(m.element)) > (b.gain, b.up, std::cmp::Reverse(b.element)),
        };
        if wins {
            *best = Some(m);
        }
    }
}

/// The smallest barrier of any occurrence of `b` (`i64::MAX` for the void): an impact below it
/// cannot move anything out of `b`.
pub fn min_barrier(b: &Block) -> i64 {
    let is = b.internal_fit();
    let bs = binding(is);
    b.holding().iter().map(|&h| (bs - binding(is - i64::from(h))).max(0)).min().unwrap_or(i64::MAX)
}

/// Add `n` quanta to `out`, split evenly over the faces in `exits` (not empty); the remainder goes
/// one each to the faces after the `rotor`, which then points at the last one served.
#[inline(always)]
fn deal(rotor: &mut u8, exits: u8, n: u32, out: &mut [u64; 6]) {
    let k = exits.count_ones();
    if k == 1 {
        out[exits.trailing_zeros() as usize] += u64::from(n);
        return;
    }
    let (each, mut rest) = (u64::from(n / k), n % k);
    for (s, o) in out.iter_mut().enumerate() {
        if exits & (1 << s) != 0 {
            *o += each;
        }
    }
    let mut r = *rotor as usize;
    while rest > 0 {
        r = if r == 5 { 0 } else { r + 1 };
        if exits & (1 << r) != 0 {
            out[r] += 1;
            rest -= 1;
        }
    }
    *rotor = r as u8;
}

/// What the law caches per material (all derived from its configuration).
#[derive(Clone, Copy)]
struct Info {
    binding: i64,
    barrier: i64,
    len: usize,
}

/// The universe: matter, six quanta counts per cell (`q[c][d]`: heading for face `d`) and a
/// rotor per cell (the last face it dealt a remainder quantum to).
pub struct Zeldovich {
    matter: Matter,
    q: Vec<[u32; 6]>,
    rotor: Vec<u8>,
    // Scratch, derived or rebuilt every tick: not state.
    inbound: Vec<[u32; 6]>,
    open: Vec<u8>,
    info: Vec<Info>,
    /// The smallest barrier in the palette: no face struck by less can react.
    least: i64,
    /// Cells holding quanta at the start of the next tick, one bit each.
    hot: Vec<u64>,
    /// Cells that quanta entered during this tick.
    arrived: Vec<u64>,
    claim: Vec<u32>,
    faces: Vec<u64>,
    memo: HashMap<(Mat, Element, bool), Mat>,
    epoch: u32,
    work: i64,
    reactions: u64,
    active: usize,
}

impl Zeldovich {
    /// The universe at rest (no quanta) on a box of matter.
    pub fn new(matter: Matter) -> Self {
        let n = matter.grid.len();
        let mut z = Self {
            matter,
            q: vec![[0; 6]; n],
            rotor: vec![0; n],
            inbound: vec![[0; 6]; n],
            open: vec![0; n],
            info: Vec::new(),
            least: i64::MAX,
            hot: vec![0; n.div_ceil(64)],
            arrived: vec![0; n.div_ceil(64)],
            claim: vec![0; n],
            faces: Vec::new(),
            memo: HashMap::new(),
            epoch: 0,
            work: 0,
            reactions: 0,
            active: 0,
        };
        z.learn();
        for c in 0..n {
            z.reopen(c);
        }
        z
    }

    /// Cache readings for palette entries added since the last call.
    fn learn(&mut self) {
        for m in self.info.len()..self.matter.palette_len() {
            let b = self.matter.block(m as Mat);
            let barrier = min_barrier(b);
            self.least = self.least.min(barrier);
            self.info.push(Info { binding: binding(b.internal_fit()), barrier, len: b.len() });
        }
    }

    /// Recompute which faces of cell `c` conduct.
    fn reopen(&mut self, c: usize) {
        let g = self.matter.grid;
        let m = self.matter.get(c);
        let mut bits = 0u8;
        if m != VOID {
            for d in 0..6 {
                if let Some(j) = g.step(c, d) {
                    let n = self.matter.get(j);
                    if n != VOID && self.matter.bond(m, n) > 0 {
                        bits |= 1 << d;
                    }
                }
            }
        }
        self.open[c] = bits;
    }

    /// The six quanta counts of cell `c`.
    pub fn channels(&self, c: usize) -> [u32; 6] {
        self.q[c]
    }

    /// Quanta in cell `c`.
    pub fn quanta(&self, c: usize) -> u64 {
        self.q[c].iter().map(|&v| u64::from(v)).sum()
    }

    /// Which faces of cell `c` conduct (bit `d` for direction `d`).
    pub fn open(&self, c: usize) -> u8 {
        self.open[c]
    }

    /// Moves made in the last tick.
    pub fn reactions(&self) -> u64 {
        self.reactions
    }

    /// Put `n` quanta into cell `c` heading for face `d`: a directed kick, booked as work.
    pub fn kick(&mut self, c: usize, d: usize, n: u32) {
        if self.matter.get(c) == VOID || n == 0 {
            return;
        }
        self.q[c][d] = self.q[c][d].checked_add(n).expect("channel overflow");
        self.work += i64::from(n);
        self.list(c);
    }

    /// Make sure cell `c` is processed in the next tick.
    #[inline(always)]
    fn list(&mut self, c: usize) {
        self.hot[c >> 6] |= 1 << (c & 63);
    }

    /// `n` quanta enter cell `c` through face `f` this tick.
    #[inline(always)]
    fn enter(&mut self, c: usize, f: usize, n: u32) {
        if n == 0 {
            return;
        }
        self.inbound[c][f] = self.inbound[c][f].checked_add(n).expect("channel overflow");
        self.arrived[c >> 6] |= 1 << (c & 63);
    }

    /// The material after removing (`add = false`) or adding one occurrence of `e`.
    fn changed(&mut self, m: Mat, e: Element, add: bool) -> Mat {
        if let Some(&r) = self.memo.get(&(m, e, add)) {
            return r;
        }
        let mut v: Vec<Element> = self.matter.block(m).elements().to_vec();
        if add {
            v.push(e);
        } else {
            let k = v.iter().position(|&x| x == e).expect("element present");
            v.remove(k);
        }
        v.sort_unstable();
        let r = if v.is_empty() { VOID } else { self.matter.intern(&Block::new(&v).expect("within capacity")) };
        self.memo.insert((m, e, add), r);
        self.learn();
        r
    }

    /// Whether some move at face `(lo, hi)` could be afforded by `h` quanta (an exact prefilter).
    #[inline]
    fn may_react(&self, lo: usize, hi: usize, h: i64) -> bool {
        let (a, b) = (&self.info[self.matter.get(lo) as usize], &self.info[self.matter.get(hi) as usize]);
        (a.barrier <= h && b.len < CAPACITY) || (b.barrier <= h && a.len < CAPACITY)
    }

    /// Restart the claim stamps before they wrap.
    fn rewind_epochs(&mut self) {
        self.claim.fill(0);
        self.epoch = 0;
    }

    /// Phase 1: the faces struck this tick, in canonical order, each making at most one move.
    fn exchange(&mut self) {
        let g = self.matter.grid;
        let (nx, ny, nz) = (g.nx as usize, g.ny as usize, g.nz as usize);
        self.faces.clear();
        // A face that can react takes an impact of at least `least`, so one of its two sides
        // carries at least half of that: only such channels need looking at.
        let strong = (self.least.max(1) as u64).div_ceil(2);
        for w in 0..self.hot.len() {
            let mut bits = self.hot[w];
            while bits != 0 {
                let c = w * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let qc = self.q[c];
                if qc.iter().all(|&n| u64::from(n) < strong) {
                    continue;
                }
                let (x, y, z) = (c % nx, (c / nx) % ny, c / (nx * ny));
                for (d, &n) in qc.iter().enumerate() {
                    if u64::from(n) < strong {
                        continue;
                    }
                    let inside = match d {
                        0 => x > 0,
                        1 => y > 0,
                        2 => z > 0,
                        3 => x + 1 < nx,
                        4 => y + 1 < ny,
                        _ => z + 1 < nz,
                    };
                    if !inside {
                        continue;
                    }
                    let j = (c as isize + g.stride(d)) as usize;
                    let (lo, hi, axis) = if d >= 3 { (c, j, d - 3) } else { (j, c, d) };
                    let h = i64::from(self.q[lo][axis + 3]) + i64::from(self.q[hi][axis]);
                    if self.may_react(lo, hi, h) {
                        self.faces.push(lo as u64 * 3 + axis as u64);
                    }
                }
            }
        }
        self.faces.sort_unstable();
        self.faces.dedup();
        let e = self.epoch;
        for k in 0..self.faces.len() {
            let key = self.faces[k];
            let (lo, axis) = ((key / 3) as usize, (key % 3) as usize);
            let hi = (lo as isize + g.stride(axis + 3)) as usize;
            if self.claim[lo] == e || self.claim[hi] == e {
                continue;
            }
            let h = i64::from(self.q[lo][axis + 3]) + i64::from(self.q[hi][axis]);
            let (ml, mh) = (self.matter.get(lo), self.matter.get(hi));
            let Some(mv) = react(self.matter.block(ml), self.matter.block(mh), h) else { continue };
            self.claim[lo] = e;
            self.claim[hi] = e;
            self.reactions += 1;
            let (src, dst) = if mv.up { (lo, hi) } else { (hi, lo) };
            let (ms, md) = (self.matter.get(src), self.matter.get(dst));
            let ns = self.changed(ms, mv.element, false);
            let nd = self.changed(md, mv.element, true);
            self.matter.set(src, ns);
            self.matter.set(dst, nd);
            for c in [lo, hi] {
                self.reopen(c);
                for d in 0..6 {
                    if let Some(j) = g.step(c, d) {
                        self.reopen(j);
                    }
                }
            }
            // The impact is spent; impact plus release enters both cells through the face.
            self.q[lo][axis + 3] = 0;
            self.q[hi][axis] = 0;
            let t = h + mv.release;
            let (down, up) = ((t / 2) as u32, (t - t / 2) as u32);
            if ns == VOID {
                // An emptied cell hands its quanta and its share to the cell that took its last
                // occurrence.
                let carried = std::mem::take(&mut self.q[src]);
                for (d, &v) in carried.iter().enumerate() {
                    self.q[dst][d] = self.q[dst][d].checked_add(v).expect("channel overflow");
                }
                self.list(dst);
                let face = if dst == hi { axis } else { axis + 3 };
                self.enter(dst, face, down + up);
            } else {
                self.enter(lo, axis + 3, down);
                self.enter(hi, axis, up);
            }
        }
    }

    /// Phase 2a: every quantum of cell `c` crosses its face into the neighbour, or is turned back.
    #[inline(always)]
    fn cross(&mut self, c: usize) {
        let qc = std::mem::take(&mut self.q[c]);
        let open = self.open[c];
        let g = self.matter.grid;
        for (d, &n) in qc.iter().enumerate() {
            if open & (1 << d) != 0 {
                let j = (c as isize + g.stride(d)) as usize;
                self.enter(j, opposite(d), n);
            } else {
                self.enter(c, d, n);
            }
        }
    }

    /// Phase 2b: cell `c` deals what entered it, face by face in canonical order.
    #[inline]
    fn settle(&mut self, c: usize) {
        let inb = std::mem::take(&mut self.inbound[c]);
        let open = self.open[c];
        let mut out = [0u64; 6];
        let mut rotor = self.rotor[c];
        for (f, &n) in inb.iter().enumerate() {
            if n == 0 {
                continue;
            }
            let exits = open & !(1 << f);
            if exits == 0 {
                out[opposite(f)] += u64::from(n);
            } else {
                deal(&mut rotor, exits, n, &mut out);
            }
        }
        self.rotor[c] = rotor;
        self.q[c] = out.map(|v| u32::try_from(v).expect("channel overflow"));
    }
}

impl Candidate for Zeldovich {
    fn id(&self) -> &'static str {
        "zeldovich"
    }

    fn matter(&self) -> &Matter {
        &self.matter
    }

    fn step(&mut self) {
        if self.epoch == u32::MAX {
            self.rewind_epochs();
        }
        self.epoch += 1;
        self.reactions = 0;
        self.exchange();
        let mut active = 0;
        for w in 0..self.hot.len() {
            let mut bits = std::mem::take(&mut self.hot[w]);
            active += bits.count_ones() as usize;
            while bits != 0 {
                self.cross(w * 64 + bits.trailing_zeros() as usize);
                bits &= bits - 1;
            }
        }
        for w in 0..self.arrived.len() {
            let mut bits = self.arrived[w];
            while bits != 0 {
                self.settle(w * 64 + bits.trailing_zeros() as usize);
                bits &= bits - 1;
            }
        }
        self.active = active;
        std::mem::swap(&mut self.hot, &mut self.arrived);
    }

    fn poke(&mut self, i: usize, strength: i32) {
        if self.matter.get(i) == VOID || strength == 0 {
            return;
        }
        let n = strength.unsigned_abs();
        if strength > 0 {
            // Heat: dealt over the cell's conducting faces (all six in a cell with none).
            let open = self.open[i];
            let mut share = [0u64; 6];
            deal(&mut self.rotor[i], if open == 0 { 0b11_1111 } else { open }, n, &mut share);
            for (q, s) in self.q[i].iter_mut().zip(share) {
                *q = u32::try_from(u64::from(*q) + s).expect("channel overflow");
            }
            self.work += i64::from(n);
            self.list(i);
        } else {
            // Cooling: take up to `n` quanta, faces in canonical order.
            let mut left = n;
            for d in 0..6 {
                let take = left.min(self.q[i][d]);
                self.q[i][d] -= take;
                left -= take;
            }
            self.work -= i64::from(n - left);
        }
    }

    fn show(&self, i: usize) -> u8 {
        self.quanta(i).min(255) as u8
    }

    fn ledger(&self) -> Ledger {
        let quanta: i64 = self.q.iter().flatten().map(|&v| i64::from(v)).sum();
        let bound: i64 = self.matter.cells().iter().map(|&m| self.info[m as usize].binding).sum();
        Ledger::default()
            .with("quanta", quanta)
            .with("binding", bound)
            .with("work", self.work)
            .with("energy", quanta - bound - self.work)
            .with("occurrences", self.matter.occurrences())
            .with("reactions", self.reactions as i64)
    }

    fn active(&self) -> usize {
        self.active
    }

    fn state_bytes_per_cell(&self) -> usize {
        std::mem::size_of::<[u32; 6]>() + 1
    }

    fn hash_state(&self, h: &mut Fnv64) {
        for c in &self.q {
            for v in c {
                h.bytes(&v.to_le_bytes());
            }
        }
        h.bytes(&self.rotor);
    }
}

/// The lab's entry for this candidate.
pub const ENTRY: Entry = Entry {
    id: "zeldovich",
    name: "Zeldovich",
    summary: "Energy is a count of quanta heading for faces; the quanta striking a face can carry one occurrence across it, paying its barrier and taking or giving its binding, so one conserved energy covers chemistry, heat, sound and signals.",
    build: |m| Box::new(Zeldovich::new(m)),
};
