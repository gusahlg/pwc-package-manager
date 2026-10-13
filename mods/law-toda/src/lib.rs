//! Candidate law `toda`: the reversible wave field. Every matter cell is a mass on springs. It
//! remembers an integer displacement `u` and momentum `p`, and one tick is three integer shears,
//! the leapfrog (Störmer–Verlet) of a lattice of springs:
//!
//! 1. **half kick** `p_i += Σ_j F(k_ij, u_j − u_i)`, over the bonded faces of `i`;
//! 2. **drift** `u_i += G(w_i, p_i)`;
//! 3. **half kick** again, with the new `u`.
//!
//! A shear changes one half of the state by a function of the other half only, so it is undone by
//! subtracting the same function: the tick is a bijection of the whole state space, for every
//! state there is. Inside a shear the update order does not matter (each reads only the half it
//! does not write), so the law is synchronous without a second buffer. Because the tick is
//! palindromic and both `F` and `G` are odd, it is also **time-symmetric**: negating every
//! momentum turns `step` into its exact inverse, so `unstep = flip ∘ step ∘ flip`. Each half kick
//! is computed per bond and applied with opposite signs to both ends, so Σp is conserved exactly.
//!
//! **The torus.** `u` is a phase: an integer modulo 2^32, and only differences of `u` enter a
//! force. The spring [`spring_shape`] is a smooth (C²) odd function of the wrapped difference with
//! period 2^32: linear for small strain, hardening like FPUT-β, peaking, then softening to zero at
//! half a period, where the bond breaks and beyond which it pulls toward the next well. So
//! wrapping arithmetic is not an overflow accident but the geometry: every shear is a bijection of
//! the torus and no state is special. This is the Chirikov standard map (`p += K sin θ; θ += p`)
//! with one coupling per bonded face, and Frenkel–Kontorova's periodic bond.
//!
//! **Constants from fit.** The stiffness `k_ij` of a face is the bond when positive (void,
//! repulsive and neutral faces carry no spring). The inverse mass is `w = 2^21 / len` (mass is the
//! occurrence count; a full block of 32 moves `p` per tick). The time shift is the one universal
//! scale: the smallest that keeps the stiffest spring the fit table can make, on the lightest
//! block, stable at the spring's steepest point.

use pwc_mod_api::material;

use crate::material::{fit_raw, Element, Fnv64, QUANTUM};
use pwc_law_core::{Candidate, Entry, Ledger, Mat, Matter, VOID};

/// Fixed-point shift of the inverse mass: a block of `len` occurrences moves `p · w >> 16` per
/// tick, with `w = round(2^21 / len)`.
pub const MASS_SHIFT: u32 = 16;
/// The inverse-mass numerator: `2^21`, so 32 occurrences (a full block) have `w = 2^16`.
const MASS_ONE: i64 = 1 << 21;
/// Fixed-point bits of the reduced strain `t` inside the spring polynomial.
const Q: u32 = 28;

/// The candidate's entry for the physics lab.
pub const ENTRY: Entry = Entry {
    id: "toda",
    name: "Toda",
    summary: "Each block is a mass on springs (stiffness = bond, mass = occurrences) holding a displacement phase and a momentum; one tick is a palindrome of exact integer shears, so the universe runs backwards bit for bit, and a momentum flip is time reversal.",
    build: |m| Box::new(Toda::new(m)),
};

/// `x / 2^s`, rounded half away from zero: odd, so `-x` maps to the negation.
#[inline]
fn shr_round(x: i64, s: u32) -> i64 {
    // Branch-free: round the magnitude, then restore the sign (`m` is 0 or −1).
    let m = x >> 63;
    let r = (((x ^ m) - m) + (1 << (s - 1))) >> s;
    (r ^ m) - m
}

/// The spring's shape on the torus, `2^31 · h(|d| / 2^31)` with the sign of `d`, where
/// `h(t) = t + 6t³ − 13t⁴ + 6t⁵` on half a period. `h(0) = h(1) = 0` and `h''(0) = h''(1) = 0`, so
/// its odd, periodic extension is C²; `h'(0) = 1`, its slope rises to 1.46 at `t = 0.3`
/// (hardening), it peaks at `t ≈ 0.61` (the bond's strength) and falls to zero at `t = 1`.
#[inline]
pub fn spring_shape(d: i32) -> i64 {
    let h = shape_of(i64::from(d.unsigned_abs()));
    if d < 0 { -h } else { h }
}

/// [`spring_shape`] of a magnitude `a ≤ 2^31`. The linear term is exact in every bit; the
/// correction `t²(6 − 13t + 6t²)` is evaluated at `Q` bits with four multiplications.
#[inline]
fn shape_of(a: i64) -> i64 {
    let t = a >> (31 - Q);
    let t2 = (t * t) >> Q;
    let n = (t2 * ((6 << Q) - 13 * t + 6 * t2)) >> Q;
    a + ((a * n) >> Q)
}

/// The potential of [`spring_shape`] for unit stiffness, `2^62 · V(t)` with
/// `V(t) = t²/2 + 3t⁴/2 − 13t⁵/5 + t⁶` (floats: a report, never a decision).
pub fn spring_potential(d: i32) -> f64 {
    let t = f64::from(d.unsigned_abs()) / 2f64.powi(31);
    let v = t * t / 2.0 + 1.5 * t.powi(4) - 2.6 * t.powi(5) + t.powi(6);
    v * 2f64.powi(62)
}

/// The largest bond the fit table can make (Q8 of `QUANTUM`), read from the table itself.
pub fn max_bond() -> i64 {
    let zero = Element([0; 4]);
    let max_fit = (0..=255u8).map(|d| fit_raw(zero, Element([d; 4]))).max().unwrap_or(0);
    i64::from(max_fit) * 256 / i64::from(QUANTUM)
}

/// The smallest time shift that keeps the stiffest possible spring on the lightest possible block
/// (one occurrence) stable with all six faces bonded at the spring's steepest point (slope < 3/2).
/// Leapfrog is stable while `ω < 2`, and Gershgorin bounds `ω²` by `2 · 6 · (3/2) · k · w`, so
/// the shift is the least `s` with `18 · k · w < 4 · 2^(s + 16)`.
pub fn time_shift() -> u32 {
    let worst = 18 * max_bond() * MASS_ONE;
    let mut shift = 1;
    while worst >= 4 << (shift + MASS_SHIFT) {
        shift += 1;
    }
    shift
}

/// The reversible wave field on a box of matter.
#[derive(Clone)]
pub struct Toda {
    matter: Matter,
    u: Vec<i32>,
    p: Vec<i32>,
    /// Bit `d`: the face in direction `d` carries a spring.
    links: Vec<u8>,
    /// Palette × palette spring stiffness, `max(bond, 0)`.
    stiff: Vec<i32>,
    /// Inverse mass per palette entry.
    weight: Vec<i32>,
    shift: u32,
    strides: [isize; 6],
    /// Cells that may need work: a superset of those with `p ≠ 0` or a non-zero force. Every
    /// entry is marked with the current era. The lists grow to the largest activity seen and are
    /// reused, so a tick allocates nothing.
    hot: Vec<u32>,
    spare: Vec<u32>,
    moved: Vec<u32>,
    mark: Vec<u32>,
    era: u32,
    /// Sweep every cell in index order instead of the hot list (a cache: chosen when most of the
    /// box is busy, never changes the result).
    dense: bool,
    /// The half-tick impulse on each cell at the current `u` (a cache of a function of `u`).
    half: Vec<i32>,
    /// Matter cells in the box.
    filled: usize,
    kicks: i64,
    worked: usize,
}

impl Toda {
    /// The field at rest (`u = p = 0`) on a box of matter.
    pub fn new(matter: Matter) -> Self {
        let g = matter.grid;
        let n = matter.palette_len();
        let mut stiff = vec![0; n * n];
        for a in 0..n {
            for b in 0..n {
                stiff[a * n + b] = matter.bond(a as Mat, b as Mat).max(0);
            }
        }
        let weight = (0..n)
            .map(|m| match i64::from(matter.reading(m as Mat).len) {
                0 => 0,
                len => ((MASS_ONE + len / 2) / len) as i32,
            })
            .collect();
        let mut links = vec![0u8; g.len()];
        for (i, l) in links.iter_mut().enumerate() {
            let a = matter.get(i) as usize;
            if a == VOID as usize {
                continue;
            }
            for (d, nb) in g.neighbours(i).into_iter().enumerate() {
                if let Some(j) = nb
                    && stiff[a * n + matter.get(j) as usize] > 0
                {
                    *l |= 1 << d;
                }
            }
        }
        let len = g.len();
        let filled = matter.cells().iter().filter(|&&m| m != VOID).count();
        Self {
            u: vec![0; len],
            p: vec![0; len],
            links,
            stiff,
            weight,
            shift: time_shift(),
            strides: std::array::from_fn(|d| g.stride(d)),
            hot: Vec::new(),
            spare: Vec::new(),
            moved: Vec::new(),
            mark: vec![0; len],
            era: 1,
            dense: false,
            half: vec![0; len],
            filled,
            kicks: 0,
            worked: 0,
            matter,
        }
    }

    /// Displacements: phases modulo 2^32 (only differences matter).
    pub fn u(&self) -> &[i32] {
        &self.u
    }

    /// Momenta.
    pub fn p(&self) -> &[i32] {
        &self.p
    }

    /// The time shift in use (see [`time_shift`]).
    pub fn shift(&self) -> u32 {
        self.shift
    }

    /// Spring stiffness between two materials (`max(bond, 0)`).
    pub fn stiffness(&self, a: Mat, b: Mat) -> i32 {
        self.stiff[a as usize * self.matter.palette_len() + b as usize]
    }

    /// Inverse mass of a material, in units of `2^-16`.
    pub fn inverse_mass(&self, m: Mat) -> i32 {
        self.weight[m as usize]
    }

    /// The long-wave sound speed (cells per tick) along a line of one material, from the linear
    /// spring: `c = sqrt(k / 2^shift · w / 2^16)`.
    pub fn sound_speed(&self, m: Mat) -> f64 {
        let k = f64::from(self.stiffness(m, m)) / 2f64.powi(self.shift as i32);
        (k * f64::from(self.inverse_mass(m)) / 2f64.powi(MASS_SHIFT as i32)).sqrt()
    }

    /// Cells that may need work next tick.
    pub fn hot(&self) -> usize {
        if self.dense { self.u.len() } else { self.hot.len() }
    }

    /// The half-tick impulse of one spring of stiffness `k` stretched by the wrapped difference
    /// `d`: `k · shape(d) / 2^(shift + 1)`, rounded half away from zero, so odd in `d`.
    #[inline]
    fn spring(&self, k: i32, d: i32) -> i32 {
        let s = self.shift + 1;
        let h = shape_of(i64::from(d.unsigned_abs()));
        let f = ((i64::from(k) * h + (1 << (s - 1))) >> s) as i32;
        if d < 0 { -f } else { f }
    }

    /// The half-tick impulse on cell `i` from all its springs.
    #[inline]
    fn half_force(&self, i: usize) -> i32 {
        let cells = self.matter.cells();
        let row = cells[i] as usize * self.matter.palette_len();
        let ui = self.u[i];
        let mut l = self.links[i];
        let mut f = 0i32;
        while l != 0 {
            let d = l.trailing_zeros() as usize;
            l &= l - 1;
            let j = i.wrapping_add_signed(self.strides[d]);
            f += self.spring(self.stiff[row + cells[j] as usize], self.u[j].wrapping_sub(ui));
        }
        f
    }

    /// How far cell `i` moves in one tick with momentum `p`, modulo 2^32 (odd in `p`).
    #[inline]
    fn glide(&self, i: usize, p: i32) -> i32 {
        let w = self.weight[self.matter.cells()[i] as usize];
        shr_round(i64::from(p) * i64::from(w), MASS_SHIFT) as i32
    }

    /// Start a new membership era for the hot list.
    fn new_era(&mut self) {
        self.era = self.era.wrapping_add(1);
        if self.era == 0 {
            self.mark.fill(0);
            self.era = 1;
        }
    }

    /// Add `i` to the list being built in this era (once).
    #[inline]
    fn admit(mark: &mut [u32], era: u32, list: &mut Vec<u32>, i: usize) {
        if mark[i] != era {
            mark[i] = era;
            list.push(i as u32);
        }
    }

    /// Add the cells in `moved` and their bonded neighbours to `list` (this era).
    fn admit_moved(&mut self, mut list: Vec<u32>) -> Vec<u32> {
        for &i in &self.moved {
            let i = i as usize;
            Self::admit(&mut self.mark, self.era, &mut list, i);
            let mut l = self.links[i];
            while l != 0 {
                let d = l.trailing_zeros() as usize;
                l &= l - 1;
                Self::admit(&mut self.mark, self.era, &mut list, i.wrapping_add_signed(self.strides[d]));
            }
        }
        list
    }

    /// One tick forward: half kick, drift, half kick. Each spring is evaluated once per tick (the
    /// first half kick reuses the impulse cached at the end of the last one). Only hot cells and
    /// the neighbours of cells that moved are touched: a cell with `p = 0` and no force is
    /// exactly at rest. When most of the box is busy it sweeps every cell in order instead.
    pub fn step(&mut self) {
        if self.dense {
            self.step_dense();
            return;
        }
        let hot = std::mem::take(&mut self.hot);
        self.moved.clear();
        // Half kick with the cached impulse, then drift (reads only `p`).
        for &i in &hot {
            let i = i as usize;
            let p = self.p[i].wrapping_add(self.half[i]);
            self.p[i] = p;
            let du = self.glide(i, p);
            if du != 0 {
                self.u[i] = self.u[i].wrapping_add(du);
                self.moved.push(i as u32);
            }
        }
        // The hot list is still this era's: extend it with what moved and its neighbours, whose
        // springs changed.
        let candidates = self.admit_moved(hot);
        let mut keep = std::mem::take(&mut self.spare);
        keep.clear();
        self.new_era();
        // Half kick with the springs at the new `u` (reads only `u`), cached for the next tick.
        for &i in &candidates {
            let i = i as usize;
            let f = self.half_force(i);
            self.half[i] = f;
            let p = self.p[i].wrapping_add(f);
            self.p[i] = p;
            if p != 0 || f != 0 {
                Self::admit(&mut self.mark, self.era, &mut keep, i);
            }
        }
        self.worked = candidates.len();
        self.spare = candidates;
        self.hot = keep;
        self.dense = self.hot.len() * 4 > self.filled;
    }

    /// One tick backward, the exact inverse of [`step`](Self::step). The law is time-symmetric
    /// (each half of the tick mirrors the other and both the drift and the springs are odd), so
    /// the inverse is the step itself seen in time's mirror: `flip ∘ step ∘ flip`.
    pub fn unstep(&mut self) {
        self.flip_hot();
        self.step();
        self.flip_hot();
    }

    /// Negate the momentum of every cell that can have one.
    fn flip_hot(&mut self) {
        if self.dense {
            self.flip();
        } else {
            for &i in &self.hot {
                self.p[i as usize] = self.p[i as usize].wrapping_neg();
            }
        }
    }

    fn step_dense(&mut self) {
        self.moved.clear();
        for i in 0..self.u.len() {
            let p = self.p[i].wrapping_add(self.half[i]);
            self.p[i] = p;
            let du = self.glide(i, p);
            self.u[i] = self.u[i].wrapping_add(du);
        }
        // Every spring once, in index order: each bond adds its impulse to one end and takes it
        // from the other (the spring is odd, so this equals the per-cell sums exactly).
        self.half.fill(0);
        let cells = self.matter.cells();
        let n = self.matter.palette_len();
        for i in 0..self.u.len() {
            let mut l = self.links[i] & 0b111_000;
            if l == 0 {
                continue;
            }
            let row = cells[i] as usize * n;
            let ui = self.u[i];
            while l != 0 {
                let d = l.trailing_zeros() as usize;
                l &= l - 1;
                let j = i.wrapping_add_signed(self.strides[d]);
                let f = self.spring(self.stiff[row + cells[j] as usize], self.u[j].wrapping_sub(ui));
                self.half[i] += f;
                self.half[j] -= f;
            }
        }
        let mut keep = std::mem::take(&mut self.spare);
        keep.clear();
        for i in 0..self.u.len() {
            let f = self.half[i];
            let p = self.p[i].wrapping_add(f);
            self.p[i] = p;
            if p != 0 || f != 0 {
                keep.push(i as u32);
            }
        }
        self.worked = self.u.len();
        // Leave the sweep when activity has fallen: `keep` is exactly the cells with momentum or
        // force, so it is the hot list.
        if keep.len() * 8 < self.filled {
            self.dense = false;
            self.new_era();
            for &i in &keep {
                self.mark[i as usize] = self.era;
            }
            self.spare = std::mem::replace(&mut self.hot, keep);
        } else {
            self.spare = keep;
        }
    }

    /// Set a cell's displacement and momentum directly (experiments, saves). Void stays at rest.
    pub fn set(&mut self, i: usize, u: i32, p: i32) {
        if self.matter.get(i) == VOID {
            return;
        }
        self.u[i] = u;
        self.p[i] = p;
        // The cached impulses of the cell and its neighbours follow the new `u`.
        self.refresh(i);
        let mut l = self.links[i];
        while l != 0 {
            let d = l.trailing_zeros() as usize;
            l &= l - 1;
            self.refresh(i.wrapping_add_signed(self.strides[d]));
        }
    }

    /// Recompute the cached impulse of cell `i` and make sure it is hot.
    fn refresh(&mut self, i: usize) {
        self.half[i] = self.half_force(i);
        if !self.dense {
            Self::admit(&mut self.mark, self.era, &mut self.hot, i);
        }
    }

    /// Time's mirror: negate every momentum. The law is time-symmetric, so a mirrored world runs
    /// its history backwards exactly: `flip; step × n; flip` equals `unstep × n`.
    pub fn flip(&mut self) {
        for p in &mut self.p {
            *p = p.wrapping_neg();
        }
    }

    /// Total momentum: conserved exactly by every tick (springs are odd), changed only by pokes.
    pub fn momentum(&self) -> i64 {
        self.p.iter().map(|&p| i64::from(p)).sum()
    }

    /// Momentum added by pokes since the start.
    pub fn kicks(&self) -> i64 {
        self.kicks
    }

    /// The energy of cell `i`: its kinetic energy plus half of each spring it holds.
    pub fn cell_energy(&self, i: usize) -> f64 {
        let m = self.matter.get(i);
        if m == VOID {
            return 0.0;
        }
        let mut e = self.kinetic(i);
        let mut l = self.links[i];
        while l != 0 {
            let d = l.trailing_zeros() as usize;
            l &= l - 1;
            e += self.bond_energy(i, i.wrapping_add_signed(self.strides[d])) / 2.0;
        }
        e
    }

    fn kinetic(&self, i: usize) -> f64 {
        let w = f64::from(self.weight[self.matter.get(i) as usize]) / 2f64.powi(MASS_SHIFT as i32);
        let p = f64::from(self.p[i]);
        p * p * w / 2.0
    }

    fn bond_energy(&self, i: usize, j: usize) -> f64 {
        let k = f64::from(self.stiffness(self.matter.get(i), self.matter.get(j)));
        k / 2f64.powi(self.shift as i32) * spring_potential(self.u[j].wrapping_sub(self.u[i]))
    }

    /// The books in floating point: kinetic, potential, total `H` and the shadow energy
    /// `H̃ = H − ⅛ Σ w_i F_i²` (with `F_i` the full-tick force) that kick-drift-kick conserves far
    /// better than `H` (for linear springs in real numbers it is exactly invariant).
    pub fn energy(&self) -> Energy {
        let (mut kinetic, mut potential, mut cross) = (0.0, 0.0, 0.0);
        let scale = 2f64.powi(MASS_SHIFT as i32);
        for i in 0..self.u.len() {
            let m = self.matter.get(i);
            if m == VOID {
                continue;
            }
            let w = f64::from(self.weight[m as usize]) / scale;
            let f = 2.0 * f64::from(self.half[i]);
            kinetic += self.kinetic(i);
            cross += w * f * f / 8.0;
            for d in 3..6 {
                if self.links[i] & (1 << d) != 0 {
                    potential += self.bond_energy(i, i.wrapping_add_signed(self.strides[d]));
                }
            }
        }
        Energy { kinetic, potential, total: kinetic + potential, shadow: kinetic + potential - cross }
    }
}

/// The energy books (floats: a report, never a decision).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Energy {
    /// `Σ p² / 2m`.
    pub kinetic: f64,
    /// `Σ` spring potentials.
    pub potential: f64,
    /// `kinetic + potential`.
    pub total: f64,
    /// The shadow Hamiltonian of the kick-drift-kick map.
    pub shadow: f64,
}

impl Candidate for Toda {
    fn id(&self) -> &'static str {
        "toda"
    }

    fn matter(&self) -> &Matter {
        &self.matter
    }

    fn step(&mut self) {
        Toda::step(self);
    }

    fn unstep(&mut self) -> bool {
        Toda::unstep(self);
        true
    }

    /// Kick the momentum of cell `i` by `strength` (void ignores it). A poke is itself a shear,
    /// so `poke(i, -s)` undoes `poke(i, s)`.
    fn poke(&mut self, i: usize, strength: i32) {
        if self.matter.get(i) == VOID || strength == 0 {
            return;
        }
        self.p[i] = self.p[i].wrapping_add(strength);
        self.kicks += i64::from(strength);
        if !self.dense {
            Self::admit(&mut self.mark, self.era, &mut self.hot, i);
        }
    }

    /// Energy on a log scale: 8 steps per doubling above 2^24 units.
    fn show(&self, i: usize) -> u8 {
        let e = self.cell_energy(i);
        if e < 2f64.powi(24) { 0 } else { (8.0 * (e.log2() - 23.0)).clamp(1.0, 255.0) as u8 }
    }

    fn ledger(&self) -> Ledger {
        Ledger::default()
            .with("momentum", self.momentum())
            .with("kicks", self.kicks)
            .with("shadow-energy", self.energy().shadow.round() as i64)
            .with("hot", self.hot() as i64)
    }

    fn active(&self) -> usize {
        self.worked
    }

    fn state_bytes_per_cell(&self) -> usize {
        8
    }

    fn hash_state(&self, h: &mut Fnv64) {
        for (&u, &p) in self.u.iter().zip(&self.p) {
            h.bytes(&u.to_le_bytes());
            h.bytes(&p.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests;
