//! Candidate law `belousov`: the excitable medium. Every matter cell remembers one phase
//! `φ ∈ Z/P`: 0 is rest, 1 is excited (one tick: a firing is an event) and `2..P` is refractory.
//! The one rule, applied to every cell at once (synchronous, from the old phases):
//!
//! - a resting cell **fires** when its glow, raised by the bonds to its excited neighbours, crosses
//!   the line where the game's matter starts to shine: `glow + Σ bond(self, excited) > GLOW_FROM`;
//! - a firing or refractory cell advances `φ + 1 mod P`.
//!
//! Positive bonds excite and negative bonds inhibit, with no extra rule. Matter that already shines
//! (the game's light emitters) crosses the line with no help, so it fires whenever it rests: a
//! pacemaker. The refractory length is one tick plus one per `QUANTUM` of cohesion. Void never
//! fires. Nothing is conserved: the drive is the free recovery of every cell to rest.
//!
//! Only cells next to an excitation, pacemakers and non-resting cells are visited, so a world at
//! rest costs nothing per tick.

use pwc_mod_api::material;

use crate::material::Fnv64;
use pwc_law_core::{Candidate, Entry, Ledger, Mat, Matter, Reading, VOID};

#[cfg(test)]
mod tests;

/// The glow reading (Q8 of `QUANTUM`) above which the game's matter emits light
/// (`material::observe`, `GLOW_FROM = 7 · 64`). Here it is the firing line.
pub const GLOW_FROM: i32 = 7 * 64;

/// Cohesion (Q8 of `QUANTUM`) worth one extra refractory tick: one `QUANTUM`.
pub const RECOVERY: i32 = 256;

/// A candidate is marked in its own phase byte while a tick gathers inputs (phases stay below it).
const MARK: u8 = 0x80;

/// How far a material's glow lies above the firing line: positive means it fires with no input.
pub fn drive(r: &Reading) -> i32 {
    r.glow - GLOW_FROM
}

/// A material's cycle length `P`: one excited tick, then `1 + cohesion / QUANTUM` refractory ticks.
pub fn period(r: &Reading) -> u8 {
    (3 + r.cohesion.max(0) / RECOVERY) as u8
}

/// The excitable medium on a box of matter.
pub struct Belousov {
    matter: Matter,
    /// Per material: [`drive`] (void never fires: `i32::MIN / 2`).
    drive: Vec<i32>,
    /// Per material: [`period`].
    period: Vec<u8>,
    /// Per cell: the phase.
    phase: Vec<u8>,
    /// Cells with `φ ≠ 0`, each once.
    live: Vec<u32>,
    /// Cells set to `φ = 1` since the last tick (an entry may be stale after a poke).
    excited: Vec<u32>,
    /// Matter that fires with no input.
    pacemakers: Vec<u32>,
    next: Vec<u32>,
    cand: Vec<u32>,
    fire: Vec<u32>,
    fired: i64,
    ticks: i64,
    active: usize,
}

impl Belousov {
    /// The medium at rest on `matter`.
    pub fn new(matter: Matter) -> Self {
        let n = matter.palette_len();
        let mut drive_of = Vec::with_capacity(n);
        let mut period_of = Vec::with_capacity(n);
        for m in 0..n {
            let r = matter.reading(m as Mat);
            drive_of.push(if m as Mat == VOID { i32::MIN / 2 } else { drive(r) });
            period_of.push(period(r));
        }
        let pacemakers = (0..matter.grid.len())
            .filter(|&i| drive_of[matter.get(i) as usize] > 0)
            .map(|i| i as u32)
            .collect();
        let cells = matter.grid.len();
        Self {
            matter,
            drive: drive_of,
            period: period_of,
            phase: vec![0; cells],
            live: Vec::new(),
            excited: Vec::new(),
            pacemakers,
            next: Vec::new(),
            cand: Vec::new(),
            fire: Vec::new(),
            fired: 0,
            ticks: 0,
            active: 0,
        }
    }

    /// The phase of cell `i`.
    #[inline]
    pub fn phase(&self, i: usize) -> u8 {
        self.phase[i]
    }

    /// Whether cell `i` is excited (it fired in the last tick).
    #[inline]
    pub fn excited(&self, i: usize) -> bool {
        self.phase[i] == 1
    }

    /// The cycle length of cell `i`'s material.
    #[inline]
    pub fn period_of(&self, i: usize) -> u8 {
        self.period[self.matter.get(i) as usize]
    }

    /// Firings since the start: the bill a fuel rule would charge.
    pub fn fired(&self) -> i64 {
        self.fired
    }

    /// The face neighbours of `i` inside the box, as `(count, cells)`.
    #[inline]
    fn around(&self, i: usize) -> (usize, [u32; 6]) {
        let g = self.matter.grid;
        let (x, y, z) = g.pos(i);
        let inside = [x > 0, y > 0, z > 0, x + 1 < g.nx, y + 1 < g.ny, z + 1 < g.nz];
        let mut out = [0u32; 6];
        let mut k = 0;
        for (d, &ok) in inside.iter().enumerate() {
            if ok {
                out[k] = (i as isize + g.stride(d)) as u32;
                k += 1;
            }
        }
        (k, out)
    }
}

impl Candidate for Belousov {
    fn id(&self) -> &'static str {
        "belousov"
    }

    fn matter(&self) -> &Matter {
        &self.matter
    }

    fn step(&mut self) {
        // 1. Candidates: resting matter next to an excitation, and resting pacemakers.
        self.cand.clear();
        for k in 0..self.excited.len() {
            let e = self.excited[k] as usize;
            if self.phase[e] != 1 {
                continue;
            }
            let (n, nb) = self.around(e);
            for &j in &nb[..n] {
                let j = j as usize;
                if self.phase[j] == 0 && self.matter.get(j) != VOID {
                    self.phase[j] = MARK;
                    self.cand.push(j as u32);
                }
            }
        }
        for &p in &self.pacemakers {
            if self.phase[p as usize] == 0 {
                self.phase[p as usize] = MARK;
                self.cand.push(p);
            }
        }
        // 2. Decide every candidate from the old phases (a mark is not an excitation).
        self.fire.clear();
        for k in 0..self.cand.len() {
            let c = self.cand[k] as usize;
            self.phase[c] = 0;
            let m = self.matter.get(c);
            let mut sum = self.drive[m as usize];
            let (n, nb) = self.around(c);
            for &j in &nb[..n] {
                if self.phase[j as usize] == 1 {
                    sum += self.matter.bond(m, self.matter.get(j as usize));
                }
            }
            if sum > 0 {
                self.fire.push(c as u32);
            }
        }
        // 3. Advance every non-resting cell; 4. the fired cells are the new excitation.
        self.next.clear();
        for &l in &self.live {
            let l = l as usize;
            let f = self.phase[l] + 1;
            if f < self.period[self.matter.get(l) as usize] {
                self.phase[l] = f;
                self.next.push(l as u32);
            } else {
                self.phase[l] = 0;
            }
        }
        for &f in &self.fire {
            self.phase[f as usize] = 1;
            self.next.push(f);
        }
        self.active = self.cand.len() + self.live.len();
        self.fired += self.fire.len() as i64;
        self.ticks += 1;
        std::mem::swap(&mut self.live, &mut self.next);
        std::mem::swap(&mut self.excited, &mut self.fire);
    }

    /// `strength > 0` excites the cell (a shock), `strength < 0` quenches it (its first refractory
    /// tick), 0 does nothing. Void ignores pokes.
    fn poke(&mut self, i: usize, strength: i32) {
        if self.matter.get(i) == VOID || strength == 0 {
            return;
        }
        let new = if strength > 0 { 1 } else { 2 };
        if self.phase[i] == 0 {
            self.live.push(i as u32);
        }
        if new == 1 && self.phase[i] != 1 {
            self.excited.push(i as u32);
        }
        self.phase[i] = new;
    }

    fn show(&self, i: usize) -> u8 {
        match self.phase[i] {
            0 => 0,
            1 => 255,
            f => {
                let p = u32::from(self.period_of(i));
                (1 + (p - u32::from(f)) * 150 / p) as u8
            }
        }
    }

    fn ledger(&self) -> Ledger {
        let excited = self.live.iter().filter(|&&l| self.phase[l as usize] == 1).count();
        Ledger::default()
            .with("excited", excited as i64)
            .with("refractory", (self.live.len() - excited) as i64)
            .with("fired", self.fired)
            .with("ticks", self.ticks)
    }

    fn active(&self) -> usize {
        self.active
    }

    fn state_bytes_per_cell(&self) -> usize {
        1
    }

    fn hash_state(&self, h: &mut Fnv64) {
        h.bytes(&self.phase);
    }
}

fn build(matter: Matter) -> Box<dyn Candidate> {
    Box::new(Belousov::new(matter))
}

/// The lab's entry for this law.
pub const ENTRY: Entry = Entry {
    id: "belousov",
    name: "Belousov: the excitable medium",
    summary: "Each cell remembers a phase (rest, excited, refractory); a resting cell fires when its glow plus the bonds to its excited neighbours crosses the line where matter starts to shine.",
    build,
};
