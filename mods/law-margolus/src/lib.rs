//! Candidate law `margolus`: the reversible particle universe (Sinai's billiards, Fredkin and
//! Toffoli's billiard-ball computer, Margolus's partitioned automaton).
//!
//! Matter is walls, and the only thing that moves is particles: one bit per open cell. One tick
//! applies one block rule ([`rule`]) to every 2×2×2 block of a partition whose offset alternates
//! between ticks. A lone particle crosses its block along the body diagonal, so it flies straight at
//! one cell per axis per tick; colliding sets of particles are permuted by the unique isotropic,
//! momentum-conserving collision; a block that holds a wall is a mirror. Every block map is its own
//! inverse, so a tick is undone by applying the same partition again ([`Margolus::unstep`]), and
//! repeating a partition reverses every velocity at once ([`Margolus::reverse`]).
//!
//! Matter enters through one reading: a configuration whose response to the light probe exceeds
//! [`CLEAR`] (the game's own onset of transparency) is open, and particles cross it without
//! colliding; every other configuration is a wall.
//!
//! Layout: bits along x in `u64` words, one row per padded `(y, z)`; the rows around the box are
//! walls, so the box is closed. A tick visits block rows, skips every 32-block word that holds no
//! particle, moves lone particles with word-wide bit operations and looks up the rest in the table.

use pwc_mod_api::material;

pub mod rule;

use pwc_law_core::{Candidate, Entry, Ledger, Matter, VOID};

use crate::material::Fnv64;
pub use crate::rule::{Collision, Reflection, Rule};

/// Light-probe response (Q8 of `QUANTUM`) above which matter is open to particles: `material::observe`
/// begins to read a configuration as transparent at exactly this response.
pub const CLEAR: i32 = 2 * 256;

/// Even bit positions: the low cell of each x pair in an aligned word.
const EVEN: u64 = 0x5555_5555_5555_5555;

/// The reversible particle universe on a box of matter.
#[derive(Clone)]
pub struct Margolus {
    matter: Matter,
    rule: Rule,
    table: &'static [u8; 65536],
    /// Words per stored row (x), rows along padded y, rows along padded z.
    wpr: usize,
    ny2: usize,
    nz2: usize,
    /// Particles: one bit per cell, `wpr` words per padded row; pad rows and bits past `nx` stay 0.
    bits: Vec<u64>,
    /// Walls and clear matter per partition phase, aligned so each x pair sits in one word
    /// (`wpr + 1` words per row).
    walls: [Vec<u64>; 2],
    clear: [Vec<u64>; 2],
    /// Offset of the next tick's partition (0 or 1 on every axis).
    phase: usize,
    ticks: i64,
    emitted: i64,
    hits: i64,
    collisions: i64,
    busy: usize,
    buf: Vec<u64>,
}

impl Margolus {
    /// The universe at rest (no particles) on `matter`.
    pub fn new(matter: Matter) -> Self {
        Self::with_rule(matter, Rule::LAW)
    }

    /// The universe at rest under another block rule (for comparisons).
    pub fn with_rule(matter: Matter, rule: Rule) -> Self {
        let g = matter.grid;
        let (nx, ny, nz) = (g.nx as usize, g.ny as usize, g.nz as usize);
        let wpr = nx.div_ceil(64);
        let (ny2, nz2) = (ny + 2, nz + 2);
        let rows = ny2 * nz2;
        let mut wall_rows = vec![!0u64; rows * wpr];
        let mut clear_rows = vec![0u64; rows * wpr];
        for z in 0..nz {
            for y in 0..ny {
                let r = (z + 1) * ny2 + (y + 1);
                for x in 0..nx {
                    let m = matter.get(g.idx(x as u32, y as u32, z as u32));
                    if m == VOID || matter.reading(m).light > CLEAR {
                        wall_rows[r * wpr + x / 64] &= !(1 << (x % 64));
                        if m != VOID {
                            clear_rows[r * wpr + x / 64] |= 1 << (x % 64);
                        }
                    }
                }
            }
        }
        let align = |src: &[u64], o: usize, fill: u64| {
            let mut out = vec![0u64; rows * (wpr + 1)];
            for r in 0..rows {
                let w = |i: usize| if i == 0 || i > wpr { fill } else { src[r * wpr + i - 1] };
                for i in 0..=wpr {
                    // `w(i + 1)` is stored word i; `w(i)` the one before it.
                    out[r * (wpr + 1) + i] = if o == 0 { w(i + 1) } else { (w(i + 1) << 1) | (w(i) >> 63) };
                }
            }
            out
        };
        let walls = [align(&wall_rows, 0, !0), align(&wall_rows, 1, !0)];
        let clear = [align(&clear_rows, 0, 0), align(&clear_rows, 1, 0)];
        Self {
            matter,
            rule,
            table: rule::table(rule),
            wpr,
            ny2,
            nz2,
            bits: vec![0; rows * wpr],
            walls,
            clear,
            phase: 0,
            ticks: 0,
            emitted: 0,
            hits: 0,
            collisions: 0,
            busy: 0,
            buf: vec![0; 8 * (wpr + 1)],
        }
    }

    /// The stored word and bit of a cell.
    #[inline]
    fn at(&self, i: usize) -> (usize, u64) {
        let (x, y, z) = self.matter.grid.pos(i);
        let r = (z as usize + 1) * self.ny2 + (y as usize + 1);
        (r * self.wpr + x as usize / 64, 1 << (x % 64))
    }

    /// Whether particles may occupy cell `i` (void or clear matter).
    pub fn is_open(&self, i: usize) -> bool {
        let m = self.matter.get(i);
        m == VOID || self.matter.reading(m).light > CLEAR
    }

    /// Whether cell `i` holds a particle.
    #[inline]
    pub fn has(&self, i: usize) -> bool {
        let (w, b) = self.at(i);
        self.bits[w] & b != 0
    }

    /// Put a particle on cell `i`; false when the cell is a wall or already occupied. Its velocity is
    /// set by where it stands ([`Margolus::velocity`]).
    pub fn put(&mut self, i: usize) -> bool {
        if !self.is_open(i) || self.has(i) {
            return false;
        }
        let (w, b) = self.at(i);
        self.bits[w] |= b;
        true
    }

    /// Remove the particle on cell `i`, if any.
    pub fn take(&mut self, i: usize) -> bool {
        let (w, b) = self.at(i);
        let had = self.bits[w] & b != 0;
        self.bits[w] &= !b;
        had
    }

    /// The velocity a particle on cell `i` has in the next tick: +1 on every axis whose coordinate
    /// has the parity of the partition offset, −1 on the others.
    pub fn velocity(&self, i: usize) -> [i32; 3] {
        let (x, y, z) = self.matter.grid.pos(i);
        [x, y, z].map(|c| 1 - 2 * ((c as usize + self.phase) & 1) as i32)
    }

    /// Particles in the box.
    pub fn particles(&self) -> i64 {
        self.bits.iter().map(|w| i64::from(w.count_ones())).sum()
    }

    /// Total momentum (sum of velocities) of all particles.
    pub fn momentum(&self) -> [i64; 3] {
        let mut p = [0i64; 3];
        let sign = |c: usize| 1 - 2 * ((c + self.phase) & 1) as i64;
        for zp in 1..self.nz2 - 1 {
            for yp in 1..self.ny2 - 1 {
                let r = zp * self.ny2 + yp;
                for i in 0..self.wpr {
                    let w = self.bits[r * self.wpr + i];
                    let n = i64::from(w.count_ones());
                    // Word starts are multiples of 64, so bit parity is x parity.
                    let even = i64::from((w & EVEN).count_ones());
                    p[0] += sign(0) * (2 * even - n);
                    p[1] += sign(yp - 1) * n;
                    p[2] += sign(zp - 1) * n;
                }
            }
        }
        p
    }

    /// Offset of the next tick's partition: 0 or 1 on every axis.
    pub fn phase(&self) -> usize {
        self.phase
    }

    /// Ticks run (forward minus backward).
    pub fn ticks(&self) -> i64 {
        self.ticks
    }

    /// Velocity components reversed by walls in the last tick: the impulse given to the walls, in
    /// units of two.
    pub fn wall_hits(&self) -> i64 {
        self.hits
    }

    /// Blocks in empty space whose particles collided in the last tick.
    pub fn collisions(&self) -> i64 {
        self.collisions
    }

    /// Particles poked in minus particles poked out.
    pub fn emitted(&self) -> i64 {
        self.emitted
    }

    /// The block rule.
    pub fn rule(&self) -> Rule {
        self.rule
    }

    /// Undo the last tick exactly: the same partition again, since every block map is an involution.
    pub fn unstep(&mut self) {
        self.phase ^= 1;
        self.apply();
        self.ticks -= 1;
    }

    /// Reverse every velocity: the next tick repeats the last partition, so the history from here on
    /// runs backward. Applying it twice changes nothing.
    pub fn reverse(&mut self) {
        self.phase ^= 1;
    }

    /// Apply the block rule to the partition of offset `phase` (in place: blocks are disjoint).
    fn apply(&mut self) {
        let o = self.phase;
        let (wpr, ny2, nz2) = (self.wpr, self.ny2, self.nz2);
        let wa = wpr + 1;
        let walls = &self.walls[o];
        let clear = &self.clear[o];
        let table = self.table;
        let bounce = self.rule.reflection == Reflection::Bounce;
        // Under the derived collision an odd number of particles always flies free.
        let odd_free = if self.rule.collision == Collision::Derived { !0 } else { 0 };
        let mut buf = std::mem::take(&mut self.buf);
        let (inp, out) = buf.split_at_mut(4 * wa);
        let (mut hits, mut collisions, mut busy) = (0i64, 0i64, 0usize);
        let start = 1 - o;
        for zp in (start..nz2 - 1).step_by(2) {
            for yp in (start..ny2 - 1).step_by(2) {
                // Corner c = dx | dy<<1 | dz<<2 lives in row dy + 2·dz.
                let rows = [zp * ny2 + yp, zp * ny2 + yp + 1, (zp + 1) * ny2 + yp, (zp + 1) * ny2 + yp + 1];
                if rows.iter().all(|&r| self.bits[r * wpr..(r + 1) * wpr].iter().all(|&w| w == 0)) {
                    continue;
                }
                for (k, &r) in rows.iter().enumerate() {
                    let src = &self.bits[r * wpr..(r + 1) * wpr];
                    let w = |i: usize| if i == 0 || i > wpr { 0 } else { src[i - 1] };
                    for i in 0..wa {
                        inp[k * wa + i] = if o == 0 { w(i + 1) } else { (w(i + 1) << 1) | (w(i) >> 63) };
                    }
                }
                for i in 0..wa {
                    let p = [inp[i], inp[wa + i], inp[2 * wa + i], inp[3 * wa + i]];
                    // Per block: any particle, at least two, an odd number.
                    let (mut one, mut two, mut odd) = (0u64, 0u64, 0u64);
                    for w in p {
                        for q in [w & EVEN, (w >> 1) & EVEN] {
                            two |= one & q;
                            one |= q;
                            odd ^= q;
                        }
                    }
                    if one == 0 {
                        for k in 0..4 {
                            out[k * wa + i] = 0;
                        }
                        continue;
                    }
                    busy += one.count_ones() as usize;
                    let wl: [u64; 4] = std::array::from_fn(|k| walls[rows[k] * wa + i]);
                    let walled = wl.iter().fold(0, |a, &w| a | ((w | w >> 1) & EVEN));
                    let exc = (two & !(odd & odd_free)) | (one & walled);
                    let free = one & !exc;
                    let keep = free | free << 1;
                    // A lone particle in empty space crosses to the opposite corner: the opposite row,
                    // the other bit of its pair.
                    for k in 0..4 {
                        let w = p[3 - k];
                        out[k * wa + i] = (((w & EVEN) << 1) | ((w >> 1) & EVEN)) & keep;
                    }
                    // Blocks holding clear matter and no wall: collisions off, flight only.
                    let glassy = (0..4).fold(0, |a, k| {
                        let c = clear[rows[k] * wa + i];
                        a | ((c | c >> 1) & EVEN)
                    }) & !walled;
                    let mut e = exc;
                    while e != 0 {
                        let j = e.trailing_zeros();
                        e &= e - 1;
                        let gather = |v: &[u64; 4]| (0..4).fold(0u8, |a, k| a | (((v[k] >> j) & 3) as u8) << (2 * k));
                        let s = gather(&p);
                        let w = gather(&wl);
                        let flown = rule::flip(s, 7);
                        let t = if (glassy >> j) & 1 == 1 { flown } else { table[(w as usize) << 8 | s as usize] };
                        if w != 0 {
                            let kept = if bounce { 0 } else { rule::mirror_axes(w).count_ones() };
                            hits += i64::from(s.count_ones() * (3 - kept));
                        } else if t != flown {
                            collisions += 1;
                        }
                        for k in 0..4 {
                            out[k * wa + i] |= u64::from((t >> (2 * k)) & 3) << j;
                        }
                    }
                }
                for (k, &r) in rows.iter().enumerate() {
                    let src = &out[k * wa..(k + 1) * wa];
                    let dst = &mut self.bits[r * wpr..(r + 1) * wpr];
                    for (i, d) in dst.iter_mut().enumerate() {
                        *d = if o == 0 { src[i] } else { (src[i] >> 1) | (src[i + 1] << 63) };
                    }
                }
            }
        }
        self.buf = buf;
        self.hits = hits;
        self.collisions = collisions;
        self.busy = busy;
    }
}

impl Candidate for Margolus {
    fn id(&self) -> &'static str {
        "margolus"
    }

    fn matter(&self) -> &Matter {
        &self.matter
    }

    fn step(&mut self) {
        self.apply();
        self.phase ^= 1;
        self.ticks += 1;
    }

    /// Exact: the inverse rule with the partitions in reverse order.
    fn unstep(&mut self) -> bool {
        Margolus::unstep(self);
        true
    }

    /// A positive strength emits particles into every open, empty cell within Chebyshev radius
    /// `strength − 1` (at most 7) of `i`; a negative strength removes them. Velocities follow from
    /// where the particles stand, so a burst flies apart in all eight diagonal directions.
    fn poke(&mut self, i: usize, strength: i32) {
        if strength == 0 {
            return;
        }
        let g = self.matter.grid;
        let r = (strength.unsigned_abs() - 1).min(7) as i32;
        let (x, y, z) = g.pos(i);
        for dz in -r..=r {
            for dy in -r..=r {
                for dx in -r..=r {
                    if let Some(j) = g.at(x as i32 + dx, y as i32 + dy, z as i32 + dz) {
                        if strength > 0 && self.put(j) {
                            self.emitted += 1;
                        } else if strength < 0 && self.take(j) {
                            self.emitted -= 1;
                        }
                    }
                }
            }
        }
    }

    fn show(&self, i: usize) -> u8 {
        if self.has(i) { 255 } else { 0 }
    }

    fn ledger(&self) -> Ledger {
        let p = self.momentum();
        Ledger::default()
            .with("particles", self.particles())
            .with("emitted", self.emitted)
            .with("momentum_x", p[0])
            .with("momentum_y", p[1])
            .with("momentum_z", p[2])
            .with("wall_hits", self.hits)
            .with("collisions", self.collisions)
    }

    /// The cells of every block that held a particle in the last tick (8 per block).
    fn active(&self) -> usize {
        8 * self.busy
    }

    /// One bit per cell, rounded up to the interface's whole byte.
    fn state_bytes_per_cell(&self) -> usize {
        1
    }

    fn hash_state(&self, h: &mut Fnv64) {
        h.bytes(&[self.phase as u8]);
        for w in &self.bits {
            h.bytes(&w.to_le_bytes());
        }
    }
}

fn build(matter: Matter) -> Box<dyn Candidate> {
    Box::new(Margolus::new(matter))
}

/// The lab's handle on this law.
pub const ENTRY: Entry = Entry {
    id: "margolus",
    name: "Margolus billiards",
    summary: "One particle bit per open cell; every 2×2×2 block of an alternating partition applies one \
              reversible rule: fly across, collide by symmetry, or mirror off walls.",
    build,
};

#[cfg(test)]
mod tests;
