//! Candidate law `polyakov`: the universe remembers relations. Every face between two cells holds an
//! integer electric flux `D`, every edge shared by four cells holds an integer magnetic flux `B`, and
//! every cell holds an integer charge `ρ`. One tick is three operators on that cell complex:
//!
//! 1. **Ohm** (the only lossy part): a face whose two cells carry free charge passes the current
//!    `J = g·(|D| − T)⁺·sgn D`. `J` leaves the face's flux and moves `J` units of charge across it.
//! 2. **Faraday**: `B −= curl E`, with `E = ε⁻¹·D` per face.
//! 3. **Ampère**: `D += curlᵀ H`, with `H = μ⁻¹·B` per edge.
//!
//! `curl` is the boundary map of the cube complex and `curlᵀ` its transpose, so `div curlᵀ = 0` and
//! `div curl = 0` hold as integer identities: Gauss's law `div D = ρ` and `div B = 0` are never
//! violated, by any tick, poke or kick. Faraday and Ampère are shears, so the field part is a
//! bijection and runs backwards exactly ([`Polyakov::step_back`]).
//!
//! The fluxes are `i16` and every operation wraps: the gauge group is `Z_65536`, Polyakov's compact
//! variant. The state space is finite, nothing can overflow, and a field driven past ±32768 tears
//! into magnetic monopole pairs (`div B` = ±65536) instead of saturating.
//!
//! The box is the whole universe: a face with a cell on one side only, or an edge missing one of its
//! four cells, relates nothing and holds nothing. That is a perfect magnetic wall (normal `D` and
//! tangential `H` vanish on the box surface). Gauss's law holds in every cell, and `div B = 0` at
//! every vertex surrounded by eight cells.
//!
//! Constants (all in 1/256, from readings, no material named):
//! - `ε⁻¹` of a cell: `128·256 / (256 + light⁺)`; of a face, the mean of its two cells. Void reads
//!   light 0, so it is the vacuum, `1/2`.
//! - `μ⁻¹`: `1/2` everywhere. An edge belongs to four cells and to no pair reading, so it is
//!   universal.
//! - `g` of a face (the share of its flux it discharges per tick): the smaller carrier mobility
//!   `clamp(glow, 0, 256)` of its two cells. 256 discharges the face fully: a perfect conductor.
//! - `T` of a face (breakdown flux): the incompatibility of its cells, `max(0, −bond)`.
//!
//! Every face and edge coefficient is at most the vacuum's, so the 3-D Yee stability bound
//! `ε⁻¹·μ⁻¹ ≤ 1/3` holds by construction (`1/4`), and every rounding is to nearest with ties toward
//! zero, so it never makes a coupling larger than its coefficient.

use pwc_mod_api::material;

use crate::material::Fnv64;
use pwc_law_core::{Candidate, Entry, Ledger, Mat, Matter, Reading};

#[cfg(test)]
mod tests;

/// The vacuum's `ε⁻¹` and the universal `μ⁻¹`, in 1/256.
pub const VACUUM: i32 = 128;
/// The order of the gauge group: fluxes and charges are integers mod this.
pub const WRAP: i64 = 1 << 16;

/// Row flags: the row holds nonzero `D`, `E` or `B`, or the law touched it this tick.
const LD: u8 = 1;
const LE: u8 = 2; // the scratch row (E or H) is nonzero
const LB: u8 = 4;
const TOUCHED: u8 = 8;

/// The constants of one face, derived from its two cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Face {
    /// `ε⁻¹` in 1/256.
    pub e: i32,
    /// Conductance `g`: the share of the flux it discharges per tick, in 1/256 (0: it insulates).
    pub s: i32,
    /// Breakdown flux `T`: the face conducts only the flux beyond it.
    pub t: i32,
}

/// `ε⁻¹` of a cell (1/256): the vacuum's, lowered by how strongly the cell attracts the light probe.
pub fn eps_inv(r: &Reading) -> i32 {
    VACUUM * 256 / (256 + r.light.max(0))
}

/// Carrier mobility of a cell (1/256 of full discharge per tick): its attraction to the glow probe,
/// clamped to `0..=256`.
pub fn mobility(r: &Reading) -> i32 {
    r.glow.clamp(0, 256)
}

/// The constants of the face between materials `a` and `b`.
pub fn face_of(m: &Matter, a: Mat, b: Mat) -> Face {
    let (ra, rb) = (m.reading(a), m.reading(b));
    Face { e: (eps_inv(ra) + eps_inv(rb)) / 2, s: mobility(ra).min(mobility(rb)), t: (-m.bond(a, b)).max(0) }
}

/// `x·k/256` rounded to nearest, ties toward zero: never larger in magnitude than the exact value.
#[inline(always)]
fn scale(x: i32, k: i32) -> i32 {
    let p = x * k;
    // (p + 127) >> 8 for p ≥ 0 and (p + 128) >> 8 for p < 0: both round half toward zero.
    (p + 127 - (p >> 31)) >> 8
}

/// A face that conducts: its cell (the + side), axis and constants.
#[derive(Clone, Copy, Debug)]
struct Wire {
    i: u32,
    k: u8,
    e: u8,
    g: u16,
    t: u16,
}

// The bench reports this size.
const _: () = assert!(std::mem::size_of::<Wire>() == 12);

/// The field universe on a box of matter.
pub struct Polyakov {
    matter: Matter,
    nx: usize,
    ny: usize,
    nz: usize,
    /// Flux through each cell's −x, −y, −z face (positive: along +axis).
    d: [Vec<i16>; 3],
    /// Flux on each cell's edge parallel to x, y, z through its (−,−) corner.
    b: [Vec<i16>; 3],
    /// Charge per cell.
    rho: Vec<i16>,
    /// Scratch: `E` of every face, then `H` of every edge, within a tick (not state).
    e: [Vec<i16>; 3],
    /// Cache of the matter: `ε⁻¹` of every face.
    eps: [Vec<u8>; 3],
    /// Cache of the matter: the conducting faces in index order, and where each row's begin.
    wires: Vec<Wire>,
    wire_rows: Vec<u32>,
    /// Face constants, `palette × palette`.
    faces: Vec<Face>,
    pal: usize,
    /// `μ⁻¹` (1/256).
    mu: i32,
    /// A row of zeros: the edges beyond the box.
    zero: Vec<i16>,
    /// Per row (y, z): which fields are nonzero there.
    live: Vec<u8>,
    /// Summed `J·E` of every current so far: the energy Ohm took out of the field.
    joule: i64,
    active: usize,
}

impl Polyakov {
    /// The universe at rest (no flux, no charge) on `matter`.
    pub fn new(matter: Matter) -> Self {
        let g = matter.grid;
        let (nx, ny, nz) = (g.nx as usize, g.ny as usize, g.nz as usize);
        let n = g.len();
        let pal = matter.palette_len();
        let mut faces = Vec::with_capacity(pal * pal);
        for a in 0..pal {
            for b in 0..pal {
                faces.push(face_of(&matter, a as Mat, b as Mat));
            }
        }
        let mut eps = [vec![VACUUM as u8; n], vec![VACUUM as u8; n], vec![VACUUM as u8; n]];
        let (mut wires, mut wire_rows) = (Vec::new(), Vec::with_capacity(ny * nz + 1));
        let cells = matter.cells();
        for i in 0..n {
            if i % nx == 0 {
                wire_rows.push(wires.len() as u32);
            }
            let at = [i % nx, (i / nx) % ny, i / (nx * ny)];
            for k in 0..3 {
                if at[k] == 0 {
                    continue;
                }
                let j = i - [1, nx, nx * ny][k];
                let f = faces[cells[i] as usize * pal + cells[j] as usize];
                eps[k][i] = f.e as u8;
                if f.s > 0 {
                    wires.push(Wire { i: i as u32, k: k as u8, e: f.e as u8, g: f.s as u16, t: f.t.min(0xffff) as u16 });
                }
            }
        }
        wire_rows.push(wires.len() as u32);
        let z = || vec![0i16; n];
        Self {
            nx,
            ny,
            nz,
            d: [z(), z(), z()],
            b: [z(), z(), z()],
            rho: z(),
            e: [z(), z(), z()],
            eps,
            wires,
            wire_rows,
            faces,
            pal,
            mu: VACUUM,
            zero: vec![0; nx],
            live: vec![0; ny * nz],
            joule: 0,
            active: 0,
            matter,
        }
    }

    /// The constants of the face across cell `i`'s −axis side (which must lie inside the box).
    pub fn face(&self, i: usize, axis: usize) -> Face {
        let j = i - self.stride(axis);
        let c = self.matter.cells();
        self.faces[c[i] as usize * self.pal + c[j] as usize]
    }

    /// Faces that conduct (the length of the cached list Ohm walks).
    pub fn conducting_faces(&self) -> usize {
        self.wires.len()
    }

    /// `ε⁻¹` (1/256) the law uses on the face across cell `i`'s −axis side.
    pub fn eps_face(&self, i: usize, axis: usize) -> i32 {
        i32::from(self.eps[axis][i])
    }

    #[inline]
    fn stride(&self, axis: usize) -> usize {
        [1, self.nx, self.nx * self.ny][axis]
    }

    #[inline]
    fn coord(&self, i: usize, axis: usize) -> usize {
        [i % self.nx, (i / self.nx) % self.ny, i / (self.nx * self.ny)][axis]
    }

    #[inline]
    fn size(&self, axis: usize) -> usize {
        [self.nx, self.ny, self.nz][axis]
    }

    /// Flux through cell `i`'s −axis face.
    pub fn flux(&self, i: usize, axis: usize) -> i16 {
        self.d[axis][i]
    }

    /// Magnetic flux on cell `i`'s edge along `axis`.
    pub fn magnetic(&self, i: usize, axis: usize) -> i16 {
        self.b[axis][i]
    }

    /// Charge of cell `i`.
    pub fn charge(&self, i: usize) -> i16 {
        self.rho[i]
    }

    /// The current Ohm will pass through cell `i`'s −axis face next tick (positive: along +axis).
    pub fn current(&self, i: usize, axis: usize) -> i32 {
        if self.coord(i, axis) == 0 {
            return 0;
        }
        let f = self.face(i, axis);
        let dv = i32::from(self.d[axis][i]);
        if f.s == 0 || dv.abs() <= f.t {
            return 0;
        }
        let a = scale(dv.abs() - f.t, f.s);
        if dv < 0 { -a } else { a }
    }

    fn mark(&mut self, i: usize, flags: u8) {
        self.live[i / self.nx] |= flags;
    }

    /// Create `+s` at `i` and `−s` at its neighbour in direction `dir` (law-core order), joined by
    /// `s` units of flux across their shared face: Gauss's law holds before and after. Chained pairs
    /// are a Wilson line, the flux string between two separated charges. `false` when the neighbour
    /// is outside the box.
    pub fn pair(&mut self, i: usize, dir: usize, s: i16) -> bool {
        let axis = dir % 3;
        let Some(j) = self.matter.grid.step(i, dir) else { return false };
        // The face belongs to the cell on its + side; positive flux runs along +axis.
        let (owner, flow) = if dir >= 3 { (j, s) } else { (i, s.wrapping_neg()) };
        self.d[axis][owner] = self.d[axis][owner].wrapping_add(flow);
        self.rho[i] = self.rho[i].wrapping_add(s);
        self.rho[j] = self.rho[j].wrapping_sub(s);
        self.mark(owner, LD);
        true
    }

    /// A magnetic kick: change `B` as if the face across cell `i`'s −axis side had held the field
    /// `a` for one tick. It changes the four edges around that face and keeps `div B = 0`.
    pub fn kick(&mut self, i: usize, axis: usize, a: i16) {
        if self.coord(i, axis) == 0 {
            return;
        }
        let (p, q) = ((axis + 1) % 3, (axis + 2) % 3);
        // E_k(i) enters curl_p at i (+) and at i + q̂ (−), curl_q at i (−) and at i + p̂ (+); B −= curl.
        for (edge, step, sign) in [(p, None, -1i16), (p, Some(q), 1), (q, None, 1), (q, Some(p), -1)] {
            let cell = match step {
                None => i,
                Some(s) if self.coord(i, s) + 1 < self.size(s) => i + self.stride(s),
                Some(_) => continue,
            };
            if self.edge_exists(cell, edge) {
                self.b[edge][cell] = self.b[edge][cell].wrapping_add(sign.wrapping_mul(a));
                self.mark(cell, LB);
            }
        }
    }

    /// Whether cell `i`'s edge along `axis` has all four of its cells in the box.
    fn edge_exists(&self, i: usize, axis: usize) -> bool {
        (0..3).filter(|&k| k != axis).all(|k| self.coord(i, k) >= 1)
    }

    /// Mark every row live, so the next tick touches every cell (benchmarks, and after edits a host
    /// cannot localise).
    pub fn wake_all(&mut self) {
        self.live.fill(LD | LE | LB);
    }

    /// Ohm on the conducting faces of every row that holds flux.
    fn ohm(&mut self) {
        let strides = [1, self.nx, self.nx * self.ny];
        let Self { d, rho, wires, wire_rows, live, joule, .. } = self;
        for (r, flags) in live.iter_mut().enumerate() {
            if *flags & LD == 0 {
                continue;
            }
            let (lo, hi) = (wire_rows[r] as usize, wire_rows[r + 1] as usize);
            if lo == hi {
                continue;
            }
            *flags |= TOUCHED;
            for w in &wires[lo..hi] {
                let (i, k) = (w.i as usize, w.k as usize);
                let dv = i32::from(d[k][i]);
                let t = i32::from(w.t);
                if dv.abs() <= t {
                    continue;
                }
                let a = scale(dv.abs() - t, i32::from(w.g));
                if a == 0 {
                    continue;
                }
                // |J| ≤ |D|: the flux decays toward zero, never past it.
                let jv = if dv < 0 { -a } else { a };
                let j = i - strides[k];
                d[k][i] = (dv - jv) as i16;
                rho[i] = rho[i].wrapping_add(jv as i16);
                rho[j] = rho[j].wrapping_sub(jv as i16);
                *joule += i64::from(jv) * i64::from(scale(dv, i32::from(w.e)));
            }
        }
    }

    /// `E = ε⁻¹·D` of every face in a row that holds flux or held field.
    fn field_e(&mut self) {
        let nx = self.nx;
        let Self { d, e, eps, live, .. } = self;
        for (r, flags) in live.iter_mut().enumerate() {
            if *flags & (LD | LE) == 0 {
                continue;
            }
            let row = r * nx..(r + 1) * nx;
            let (mut any_d, mut any_e) = (0i16, 0i16);
            for k in 0..3 {
                let (dk, ek, fk) = (&d[k][row.clone()], &mut e[k][row.clone()], &eps[k][row.clone()]);
                for ((ev, &dv), &f) in ek.iter_mut().zip(dk).zip(fk) {
                    *ev = scale(i32::from(dv), i32::from(f)) as i16;
                    any_d |= dv;
                    any_e |= *ev;
                }
            }
            *flags = (*flags & !(LD | LE)) | TOUCHED | if any_d != 0 { LD } else { 0 } | if any_e != 0 { LE } else { 0 };
        }
    }

    /// `B += sign · curl E` on every edge near a live `E` row. Only edges with four cells change.
    fn faraday(&mut self, sign: i16) {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (sy, sz) = (nx, nx * ny);
        let Self { b, e, live, .. } = self;
        let [ex, ey, ez] = &*e;
        let [bx, by, bz] = b;
        for z in 0..nz {
            for y in 0..ny {
                let r = z * ny + y;
                let near = live[r] | if y > 0 { live[r - 1] } else { 0 } | if z > 0 { live[r - ny] } else { 0 };
                if near & LE == 0 {
                    continue;
                }
                let o = r * nx;
                if y > 0 && z > 0 {
                    let (ez0, ezy, ey0, eyz) = (&ez[o..o + nx], &ez[o - sy..o - sy + nx], &ey[o..o + nx], &ey[o - sz..o - sz + nx]);
                    for ((((bv, &a), &b), &c), &d) in bx[o..o + nx].iter_mut().zip(ez0).zip(ezy).zip(ey0).zip(eyz) {
                        *bv = bv.wrapping_add(sign.wrapping_mul(a.wrapping_sub(b).wrapping_sub(c).wrapping_add(d)));
                    }
                }
                let p = o + 1;
                if z > 0 {
                    let (ex0, exz, ez0, ezl) = (&ex[p..o + nx], &ex[p - sz..o - sz + nx], &ez[p..o + nx], &ez[o..o + nx - 1]);
                    for ((((bv, &a), &b), &c), &d) in by[p..o + nx].iter_mut().zip(ex0).zip(exz).zip(ez0).zip(ezl) {
                        *bv = bv.wrapping_add(sign.wrapping_mul(a.wrapping_sub(b).wrapping_sub(c).wrapping_add(d)));
                    }
                }
                if y > 0 {
                    let (ey0, eyl, ex0, exy) = (&ey[p..o + nx], &ey[o..o + nx - 1], &ex[p..o + nx], &ex[p - sy..o - sy + nx]);
                    for ((((bv, &a), &b), &c), &d) in bz[p..o + nx].iter_mut().zip(ey0).zip(eyl).zip(ex0).zip(exy) {
                        *bv = bv.wrapping_add(sign.wrapping_mul(a.wrapping_sub(b).wrapping_sub(c).wrapping_add(d)));
                    }
                }
                let any = bx[o..o + nx].iter().chain(&by[o..o + nx]).chain(&bz[o..o + nx]).fold(0, |a, &v| a | v);
                live[r] = (live[r] & !LB) | TOUCHED | if any != 0 { LB } else { 0 };
            }
        }
    }

    /// `H = μ⁻¹·B` of every edge into the scratch (Faraday is done with `E` by now), in every row
    /// that holds `B` or a stale scratch value.
    fn field_h(&mut self) {
        let (nx, mu) = (self.nx, self.mu);
        let Self { b, e, live, .. } = self;
        for (r, flags) in live.iter_mut().enumerate() {
            if *flags & (LB | LE) == 0 {
                continue;
            }
            let row = r * nx..(r + 1) * nx;
            let mut any = 0i16;
            for k in 0..3 {
                for (hv, &bv) in e[k][row.clone()].iter_mut().zip(&b[k][row.clone()]) {
                    *hv = scale(i32::from(bv), mu) as i16;
                    any |= *hv;
                }
            }
            *flags = (*flags & !LE) | TOUCHED | if any != 0 { LE } else { 0 };
        }
    }

    /// `D += sign · curlᵀ H` on every face near a live `H` row. Edges outside the box hold nothing,
    /// so the faces on the box surface, which no edge-pair surrounds, always receive zero.
    fn ampere(&mut self, sign: i16) {
        let (nx, ny, nz) = (self.nx, self.ny, self.nz);
        let (sy, sz) = (nx, nx * ny);
        let Self { e, d, live, zero, .. } = self;
        let [hx, hy, hz] = &*e;
        let [dx, dy, dz] = d;
        for z in 0..nz {
            for y in 0..ny {
                let r = z * ny + y;
                let (up, fwd) = (y + 1 < ny, z + 1 < nz);
                let near = live[r] | if up { live[r + 1] } else { 0 } | if fwd { live[r + ny] } else { 0 };
                if near & LE == 0 {
                    continue;
                }
                let o = r * nx;
                let (hx0, hy0, hz0) = (&hx[o..o + nx], &hy[o..o + nx], &hz[o..o + nx]);
                let (hxu, hzu) = if up { (&hx[o + sy..o + sy + nx], &hz[o + sy..o + sy + nx]) } else { (&zero[..], &zero[..]) };
                let (hxf, hyf) = if fwd { (&hx[o + sz..o + sz + nx], &hy[o + sz..o + sz + nx]) } else { (&zero[..], &zero[..]) };
                // D_x += ∂y Hz − ∂z Hy
                for ((((dv, &a), &b), &c), &e) in dx[o..o + nx].iter_mut().zip(hzu).zip(hz0).zip(hyf).zip(hy0) {
                    *dv = dv.wrapping_add(sign.wrapping_mul(a.wrapping_sub(b).wrapping_sub(c).wrapping_add(e)));
                }
                // D_y += ∂z Hx − ∂x Hz and D_z += ∂x Hy − ∂y Hx; the edges right of the row are outside.
                let n1 = nx - 1;
                for ((((dv, &a), &b), &c), &e) in dy[o..o + n1].iter_mut().zip(hxf).zip(hx0).zip(&hz0[1..]).zip(hz0) {
                    *dv = dv.wrapping_add(sign.wrapping_mul(a.wrapping_sub(b).wrapping_sub(c).wrapping_add(e)));
                }
                for ((((dv, &a), &b), &c), &e) in dz[o..o + n1].iter_mut().zip(&hy0[1..]).zip(hy0).zip(hxu).zip(hx0) {
                    *dv = dv.wrapping_add(sign.wrapping_mul(a.wrapping_sub(b).wrapping_sub(c).wrapping_add(e)));
                }
                let l = o + n1;
                dy[l] = dy[l].wrapping_add(sign.wrapping_mul(hxf[n1].wrapping_sub(hx0[n1]).wrapping_add(hz0[n1])));
                dz[l] = dz[l].wrapping_add(sign.wrapping_mul(hx0[n1].wrapping_sub(hy0[n1]).wrapping_sub(hxu[n1])));
                let any = dx[o..o + nx].iter().chain(&dy[o..o + nx]).chain(&dz[o..o + nx]).fold(0, |a, &v| a | v);
                live[r] = (live[r] & !LD) | TOUCHED | if any != 0 { LD } else { 0 };
            }
        }
    }

    fn finish_tick(&mut self) {
        let mut rows = 0;
        for f in &mut self.live {
            rows += usize::from(*f & TOUCHED != 0);
            *f &= !TOUCHED;
        }
        self.active = rows * self.nx;
    }

    /// Undo one tick of the field part exactly: `D −= curlᵀ H`, then `B += curl E`. It is the exact
    /// inverse of [`Candidate::step`] wherever no current flowed in that tick (Ohm forgets).
    pub fn step_back(&mut self) {
        self.wake_all();
        self.field_h();
        self.ampere(-1);
        self.field_e();
        self.faraday(1);
        self.finish_tick();
    }

    /// Divergence of `D` at cell `i` minus its charge, as integers of the representatives.
    pub fn gauss_residual(&self, i: usize) -> i64 {
        let mut div = 0i64;
        for k in 0..3 {
            div -= i64::from(self.d[k][i]);
            if self.coord(i, k) + 1 < self.size(k) {
                div += i64::from(self.d[k][i + self.stride(k)]);
            }
        }
        div - i64::from(self.rho[i])
    }

    /// `div B` at the vertex at the (−,−,−) corner of cell `i`, defined when all eight cells around
    /// it exist (every coordinate of `i` is at least 1).
    pub fn div_b(&self, i: usize) -> Option<i64> {
        if (0..3).any(|k| self.coord(i, k) == 0) {
            return None;
        }
        let mut div = 0i64;
        for k in 0..3 {
            div += i64::from(self.b[k][i]) - i64::from(self.b[k][i - self.stride(k)]);
        }
        Some(div)
    }

    /// Field energy (twice it): `Σ ε⁻¹D² + Σ μ⁻¹B²`.
    pub fn energy(&self) -> i64 {
        let mut u = 0i64;
        for i in 0..self.rho.len() {
            for k in 0..3 {
                if self.coord(i, k) > 0 {
                    let dv = i64::from(self.d[k][i]);
                    u += i64::from(self.eps[k][i]) * dv * dv;
                }
                let bv = i64::from(self.b[k][i]);
                u += i64::from(self.mu) * bv * bv;
            }
        }
        u / 256
    }

    /// The modified energy the exact-arithmetic leapfrog conserves, `D·E + B·H − H·curl E`, of the
    /// state at the start of a tick. Rounded coefficients make it conserved only approximately.
    pub fn shadow_energy(&self) -> i64 {
        let (nx, ny) = (self.nx, self.ny);
        let n = self.rho.len();
        // Faces outside the box hold no flux, so their E is 0 whatever their coefficient.
        let ef: Vec<Vec<i64>> = (0..3)
            .map(|k| (0..n).map(|i| i64::from(scale(i32::from(self.d[k][i]), i32::from(self.eps[k][i])))).collect())
            .collect();
        let mut q = 0i64;
        for (k, row) in ef.iter().enumerate() {
            q += row.iter().zip(&self.d[k]).map(|(&ev, &dv)| ev * i64::from(dv)).sum::<i64>();
        }
        let (sy, sz) = (nx, nx * ny);
        for i in 0..n {
            let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
            for k in 0..3 {
                let bv = self.b[k][i];
                let hv = i64::from(scale(i32::from(bv), self.mu));
                q += i64::from(bv) * hv;
                let c = match k {
                    0 if y > 0 && z > 0 => ef[2][i] - ef[2][i - sy] - ef[1][i] + ef[1][i - sz],
                    1 if x > 0 && z > 0 => ef[0][i] - ef[0][i - sz] - ef[2][i] + ef[2][i - 1],
                    2 if x > 0 && y > 0 => ef[1][i] - ef[1][i - 1] - ef[0][i] + ef[0][i - sy],
                    _ => 0,
                };
                q -= hv * c;
            }
        }
        q
    }

    /// Cells where Gauss's law fails as integers, and where it fails mod [`WRAP`] (never).
    pub fn gauss_defects(&self) -> (usize, usize) {
        let mut bad = (0, 0);
        for i in 0..self.rho.len() {
            let r = self.gauss_residual(i);
            bad.0 += usize::from(r != 0);
            bad.1 += usize::from(r.rem_euclid(WRAP) != 0);
        }
        bad
    }

    /// Monopoles `m = div B / WRAP` over every vertex: `(Σ|m|, Σm, vertices where div B ≠ 0 mod WRAP)`.
    pub fn monopoles(&self) -> (i64, i64, usize) {
        let mut out = (0, 0, 0);
        for i in 0..self.rho.len() {
            if let Some(div) = self.div_b(i) {
                out.0 += (div / WRAP).abs();
                out.1 += div / WRAP;
                out.2 += usize::from(div.rem_euclid(WRAP) != 0);
            }
        }
        out
    }

    /// Total charge (summed representatives).
    pub fn total_charge(&self) -> i64 {
        self.rho.iter().map(|&q| i64::from(q)).sum()
    }

    /// Energy Ohm has taken from the field so far: `Σ J·E`, in the units of [`Self::energy`].
    pub fn joule(&self) -> i64 {
        self.joule
    }
}

impl Candidate for Polyakov {
    fn id(&self) -> &'static str {
        "polyakov"
    }

    fn matter(&self) -> &Matter {
        &self.matter
    }

    fn step(&mut self) {
        self.ohm();
        self.field_e();
        self.faraday(-1);
        self.field_h();
        self.ampere(1);
        self.finish_tick();
    }

    /// Exact only where no face conducts: Ohm forgets which flux it discharged. A box with any
    /// conducting face refuses (use [`Polyakov::step_back`] to run its field part backwards anyway).
    fn unstep(&mut self) -> bool {
        if !self.wires.is_empty() {
            return false;
        }
        self.step_back();
        true
    }

    /// A charge pair: `+strength` at `i`, `−strength` at its +x neighbour (−x at the far wall, then
    /// the other axes), with the flux string between them.
    fn poke(&mut self, i: usize, strength: i32) {
        let s = strength.clamp(-32767, 32767) as i16;
        if s == 0 {
            return;
        }
        for dir in [3, 0, 4, 1, 5, 2] {
            if self.pair(i, dir, s) {
                return;
            }
        }
    }

    fn show(&self, i: usize) -> u8 {
        let mut v = i64::from(self.rho[i]).abs();
        for k in 0..3 {
            v += i64::from(self.d[k][i]).abs() + i64::from(self.b[k][i]).abs();
        }
        if v == 0 { 0 } else { ((64 - v.leading_zeros()) * 15).min(255) as u8 }
    }

    fn ledger(&self) -> Ledger {
        let (gauss, gauss_mod) = self.gauss_defects();
        let (monopoles, monopole_sum, div_b_mod) = self.monopoles();
        Ledger::default()
            .with("charge", self.total_charge())
            .with("gauss_defects", gauss as i64)
            .with("gauss_defects_mod", gauss_mod as i64)
            .with("monopoles", monopoles)
            .with("monopole_sum", monopole_sum)
            .with("div_b_defects_mod", div_b_mod as i64)
            .with("energy", self.energy())
            .with("joule", self.joule)
    }

    fn active(&self) -> usize {
        self.active
    }

    fn state_bytes_per_cell(&self) -> usize {
        // D on three faces, B on three edges, ρ: seven i16.
        14
    }

    fn hash_state(&self, h: &mut Fnv64) {
        for v in self.d.iter().chain(self.b.iter()).chain(std::iter::once(&self.rho)) {
            for &x in v {
                h.bytes(&x.to_le_bytes());
            }
        }
    }
}

/// The lab's entry.
pub const ENTRY: Entry = Entry {
    id: "polyakov",
    name: "Polyakov",
    summary: "Integer flux on every face and edge, charge in every cell; one tick is Ohm, then the exact \
              Yee shears B −= curl E, D += curlᵀ H, so Gauss's law and div B = 0 can never break.",
    build: |m| Box::new(Polyakov::new(m)),
};
