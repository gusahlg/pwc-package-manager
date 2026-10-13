//! The experiments. Each builds matter with the deterministic search, runs the law, prints what it
//! saw under `--nocapture` and asserts the measured phenomenon.

use pwc_law_core::grid::Grid;
use crate::material::Block;
use pwc_law_core::search::{bond, find, pure, Stream};
use pwc_law_core::show::slice;

use crate::*;

/// A self-bonding mixture of `size` occurrences whose bond to itself is at least `min`.
fn springy(seed: u64, size: usize, min: i32) -> Block {
    find(seed, 2, size, 1_000_000, |b| bond(b, b) >= min)
}

/// A line of `n` cells along x, filled by `fill(x)`.
fn line(n: u32, blocks: &[Block], fill: impl Fn(u32) -> usize) -> Toda {
    let mut m = Matter::new(Grid::new(n, 1, 1));
    let ids: Vec<Mat> = blocks.iter().map(|b| m.intern(b)).collect();
    for x in 0..n {
        m.set(x as usize, ids[fill(x)]);
    }
    Toda::new(m)
}

/// Every matter cell of a box, in index order.
fn matter_cells(t: &Toda) -> Vec<usize> {
    (0..t.u.len()).filter(|&i| t.matter.get(i) != VOID).collect()
}

/// The half-tick impulses of the reference law: every cell, neighbours found through the grid and
/// bonds read straight from the matter.
fn naive_half(t: &Toda) -> Vec<i32> {
    let g = t.matter.grid;
    let mut f = vec![0i32; g.len()];
    for (i, fi) in f.iter_mut().enumerate() {
        let a = t.matter.get(i);
        if a == VOID {
            continue;
        }
        for j in g.neighbours(i).into_iter().flatten() {
            let k = t.matter.bond(a, t.matter.get(j)).max(0);
            if k > 0 {
                *fi += t.spring(k, t.u[j].wrapping_sub(t.u[i]));
            }
        }
    }
    f
}

/// The reference law, a full sweep with no caches: half kick, drift, half kick. The tracked `step`
/// must agree with it bit for bit.
fn naive_step(t: &mut Toda) {
    let kick = |t: &mut Toda| {
        let f = naive_half(t);
        for (p, f) in t.p.iter_mut().zip(&f) {
            *p = p.wrapping_add(*f);
        }
    };
    kick(t);
    for i in 0..t.u.len() {
        if t.matter.get(i) != VOID {
            let du = t.glide(i, t.p[i]);
            t.u[i] = t.u[i].wrapping_add(du);
        }
    }
    kick(t);
}

/// A slab world: three springy mixtures in layers, a pure (self-dead) layer, a void pocket and a
/// repulsive face, in an `nx × ny × nz` box.
fn slab(nx: u32, ny: u32, nz: u32) -> Toda {
    let a = springy(1, 2, 400);
    let b = springy(2, 8, 350);
    let c = springy(3, 16, 300);
    let mut s = Stream::new(9);
    let dead = pure(s.element(), 6);
    let mut m = Matter::new(Grid::new(nx, ny, nz));
    let ids = [m.intern(&a), m.intern(&b), m.intern(&c), m.intern(&dead)];
    let (nx, ny, nz) = (nx as i32, ny as i32, nz as i32);
    m.fill((0, 0, 0), (nx - 1, ny / 2 - 1, nz - 1), ids[2]);
    m.fill((0, ny / 4, 0), (nx - 1, ny / 2 - 2, nz - 1), ids[1]);
    m.fill((0, ny / 2 - 1, 0), (nx - 1, ny / 2 - 1, nz - 1), ids[0]);
    m.fill((nx / 3, 1, nz / 3), (nx / 2, ny / 4, nz / 2), VOID);
    m.fill((0, 0, 0), (nx / 4, 1, nz - 1), ids[3]);
    Toda::new(m)
}

#[test]
fn materials_derive_springs_and_masses_from_fit() {
    let t = slab(8, 8, 8);
    let m = t.matter();
    println!("time shift {} (from the fit table's largest bond)", t.shift());
    for id in 1..m.palette_len() as Mat {
        let r = m.reading(id);
        println!(
            "material {id}: len {:2} cohesion {:5} self-bond {:4} k {:4} w {:6} c {:.4} cells/tick",
            r.len,
            r.cohesion,
            m.bond(id, id),
            t.stiffness(id, id),
            t.inverse_mass(id),
            t.sound_speed(id)
        );
    }
    for a in 1..m.palette_len() as Mat {
        let row: Vec<i32> = (1..m.palette_len() as Mat).map(|b| m.bond(a, b)).collect();
        println!("bonds of {a}: {row:?}");
    }
    assert_eq!(t.shift(), 18);
    // The pure layer is acoustically dead: identical elements fit 0, so it bonds 0 to itself.
    assert_eq!(t.stiffness(4, 4), 0);
    for id in 1..4 {
        assert!(t.stiffness(id, id) > 0);
    }
}

#[test]
fn reversal_is_exact_bit_for_bit() {
    let mut t = slab(14, 12, 10);
    let g = t.matter().grid;
    // Pokes from gentle to absurd: the yield zone, and kicks large enough to wrap the momentum.
    let mut s = Stream::new(5);
    for k in 0..40 {
        let i = s.below(g.len() as u64) as usize;
        let strength = match k % 4 {
            0 => 300,
            1 => 30_000,
            2 => 3_000_000,
            _ => i32::MAX - 7,
        };
        t.poke(i, if k % 2 == 0 { strength } else { -strength });
    }
    let start = t.clone();
    let d0 = t.digest();
    assert!(!t.dense, "a few pokes start on the hot list");
    // The tracked law equals the reference full sweep, tick for tick, through both the hot-list
    // and the dense sweep.
    let mut r = t.clone();
    let mut digests = vec![d0];
    let mut modes = [0usize; 2];
    for _ in 0..600 {
        t.step();
        naive_step(&mut r);
        modes[usize::from(t.dense)] += 1;
        assert_eq!((&t.u, &t.p), (&r.u, &r.p), "activity tracking must not change the law");
        assert_eq!(t.half, naive_half(&t), "the cached impulses follow u");
        digests.push(t.digest());
    }
    for _ in 0..4400 {
        t.step();
        digests.push(t.digest());
    }
    let mid = t.digest();
    // Backward, every intermediate state matches the forward one.
    for k in (0..5000).rev() {
        t.unstep();
        modes[usize::from(t.dense)] += 1;
        assert_eq!(t.digest(), digests[k]);
    }
    println!(
        "digest start {d0:016x}, after 5000 ticks {mid:016x}, after 5000 back {:016x}; ticks on the hot list {}, dense {}",
        t.digest(),
        modes[0],
        modes[1]
    );
    assert!(modes[0] > 0 && modes[1] > 0, "both evaluation modes ran");
    assert!(!t.dense, "and the rewind came back to the hot list");
    assert_ne!(mid, d0);
    assert_eq!((&t.u, &t.p), (&start.u, &start.p));

    // Every state is reversible, not just physical ones: fill u and p with arbitrary 32-bit words.
    let mut t = slab(10, 10, 10);
    for i in matter_cells(&t) {
        let (u, p) = (s.next_u64() as i32, (s.next_u64() >> 32) as i32);
        t.set(i, u, p);
    }
    let d0 = t.digest();
    for _ in 0..500 {
        t.step();
    }
    for _ in 0..500 {
        t.unstep();
    }
    assert_eq!(t.digest(), d0, "the shears are bijections of the whole torus");
}

#[test]
fn hot_list_matches_the_full_sweep() {
    // One poke in a big slab: the hot list grows from one cell until the dense sweep takes over.
    // Both must equal the reference law bit for bit, and so must the switch between them.
    let mut t = slab(40, 40, 40);
    let g = t.matter().grid;
    t.poke(g.idx(20, 12, 20), 6_000_000);
    let mut r = t.clone();
    let mut sparse = 0;
    for _ in 0..150 {
        t.step();
        naive_step(&mut r);
        sparse += usize::from(!t.dense);
        assert_eq!((&t.u, &t.p), (&r.u, &r.p));
        assert_eq!(t.half, naive_half(&t));
    }
    println!("{sparse} of 150 ticks on the hot list before the dense sweep took over");
    assert!(sparse >= 20 && t.dense);
}

#[test]
fn time_mirror_retraces_the_past() {
    // Loschmidt's echo. The law is time-symmetric: flip every momentum, run forward, flip again,
    // and the world has run backwards to its start, bit for bit. No bookkeeping is needed.
    let mut t = slab(12, 10, 8);
    let g = t.matter().grid;
    t.poke(g.idx(6, 4, 4), 4_000_000);
    t.poke(g.idx(2, 3, 2), -2_500_000);
    let d0 = t.digest();
    let n = 2000;
    for _ in 0..n {
        t.step();
    }
    let mid = t.digest();
    t.flip();
    for _ in 0..n {
        t.step();
    }
    t.flip();
    println!("echo: start {d0:016x}, after {n} ticks {mid:016x}, mirrored back {:016x}", t.digest());
    assert_ne!(mid, d0);
    assert_eq!(t.digest(), d0);
}

#[test]
fn momentum_is_exact_and_energy_stays_bounded() {
    let mut t = slab(16, 16, 16);
    let g = t.matter().grid;
    t.poke(g.idx(8, 7, 8), 6_000_000);
    t.poke(g.idx(3, 2, 12), -2_000_000);
    t.poke(g.idx(12, 5, 3), 9_000_000);
    let e0 = t.energy();
    let (mut lo, mut hi, mut tlo, mut thi) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    let ticks = 60_000;
    let (mut first, mut last) = (0.0, 0.0);
    for k in 0..ticks {
        t.step();
        if k % 50 == 0 {
            assert_eq!(t.momentum(), t.kicks(), "Σp changes only by pokes");
            let e = t.energy();
            lo = lo.min(e.shadow);
            hi = hi.max(e.shadow);
            tlo = tlo.min(e.total);
            thi = thi.max(e.total);
            if k < ticks / 10 {
                first += e.shadow;
            }
            if k >= ticks - ticks / 10 {
                last += e.shadow;
            }
        }
    }
    let samples = (ticks / 10 / 50) as f64;
    let drift = (last - first) / samples / e0.shadow;
    println!(
        "E0 {:.0}: shadow band {:.4}%, H band {:.4}%, secular drift {:+.5}% over {ticks} ticks",
        e0.shadow,
        100.0 * (hi - lo) / e0.shadow,
        100.0 * (thi - tlo) / e0.shadow,
        100.0 * drift
    );
    println!("{}", slice(&t, 8));
    assert!((hi - lo) / e0.shadow < 0.01);
    assert!(drift.abs() < 0.002);
    assert!((thi - tlo) > (hi - lo), "the shadow energy is the better invariant");
}


/// Strain along a line: `u[x+1] − u[x]` (wrapped), as floats.
fn strain(t: &Toda) -> Vec<f64> {
    t.u.windows(2).map(|w| f64::from(w[1].wrapping_sub(w[0]))).collect()
}

/// The leading pulse of a strain profile: the front-most cell above `frac` of the global peak,
/// the local peak there, and the number of cells around it above half that peak.
fn lead(s: &[f64], frac: f64) -> (usize, f64, usize) {
    let top = s.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let front = (0..s.len()).rev().find(|&i| s[i].abs() > frac * top).unwrap_or(0);
    let mut at = front;
    while at > 0 && s[at - 1].abs() >= s[at].abs() {
        at -= 1;
    }
    let peak = s[at].abs();
    let (mut lo, mut hi) = (at, at);
    while lo > 0 && s[lo - 1].abs() > peak / 2.0 {
        lo -= 1;
    }
    while hi + 1 < s.len() && s[hi + 1].abs() > peak / 2.0 {
        hi += 1;
    }
    (at, peak, hi - lo + 1)
}

/// Normal-mode energies of a free line (cosine modes), in the law's energy units.
fn mode_energies(t: &Toda, m: Mat) -> Vec<f64> {
    let n = t.u.len();
    let k = f64::from(t.stiffness(m, m)) / 2f64.powi(t.shift() as i32);
    let w = f64::from(t.inverse_mass(m)) / 2f64.powi(MASS_SHIFT as i32);
    // Displacements relative to cell 0, unwrapped along the line.
    let mut u = vec![0.0; n];
    for x in 1..n {
        u[x] = u[x - 1] + f64::from(t.u[x].wrapping_sub(t.u[x - 1]));
    }
    (0..n)
        .map(|q| {
            let norm = if q == 0 { (1.0 / n as f64).sqrt() } else { (2.0 / n as f64).sqrt() };
            let phi = |x: usize| norm * (std::f64::consts::PI * q as f64 * (x as f64 + 0.5) / n as f64).cos();
            let uq: f64 = (0..n).map(|x| phi(x) * u[x]).sum();
            let pq: f64 = (0..n).map(|x| phi(x) * f64::from(t.p[x])).sum();
            let lam = 4.0 * (std::f64::consts::PI * q as f64 / (2.0 * n as f64)).sin().powi(2);
            0.5 * w * pq * pq + 0.5 * k * lam * uq * uq
        })
        .collect()
}

/// A Y junction: two input lines (y = 0 and y = 2) run along x to `j` and meet at `(j, 1)`; the
/// output line runs on from `(j + 1, 1)` through a link cell of material `link`.
fn junction(a: &Block, link: &Block, j: u32, out: u32) -> Toda {
    let mut m = Matter::new(Grid::new(j + 1 + out, 3, 1));
    let ia = m.intern(a);
    let il = m.intern(link);
    let j = j as i32;
    m.fill((0, 0, 0), (j, 0, 0), ia);
    m.fill((0, 2, 0), (j, 2, 0), ia);
    m.put(j, 1, 0, ia);
    m.fill((j + 1, 1, 0), (j + out as i32, 1, 0), ia);
    m.put(j + 2, 1, 0, il);
    Toda::new(m)
}


/// A right-moving (`dir = 1`) or left-moving (`-1`) Gaussian wave packet of displacement on a line.
fn packet(t: &mut Toda, x0: f64, sigma: f64, amp: f64, dir: f64) {
    for x in 0..t.u.len() {
        let m = t.matter.get(x);
        if m == VOID {
            continue;
        }
        let r = x as f64 - x0;
        let u = amp * (-r * r / (2.0 * sigma * sigma)).exp();
        let du = -r / (sigma * sigma) * u;
        let v = -dir * t.sound_speed(m) * du;
        let p = v * 2f64.powi(MASS_SHIFT as i32) / f64::from(t.inverse_mass(m));
        t.set(x, u as i32, p.round() as i32);
    }
}

/// Energy-weighted mean position over `range` of a line.
fn centroid(t: &Toda, range: std::ops::Range<usize>) -> f64 {
    let (mut s, mut w) = (0.0, 0.0);
    for x in range {
        let e = t.cell_energy(x);
        s += e * x as f64;
        w += e;
    }
    s / w
}

/// One row of a space-time picture: `cols` characters, each the largest |value| it covers.
fn ribbon(v: &[f64], cols: usize, top: f64) -> String {
    const RAMP: &[u8] = b" .:-=+*#%@";
    let per = v.len().div_ceil(cols);
    v.chunks(per)
        .map(|c| {
            let m = c.iter().fold(0.0f64, |m, x| m.max(x.abs())) / top;
            RAMP[((m * (RAMP.len() - 1) as f64).round() as usize).min(RAMP.len() - 1)] as char
        })
        .collect()
}

#[test]
fn sound_speed_follows_stiffness_over_mass() {
    let mats = [springy(1, 2, 400), springy(2, 8, 350), springy(3, 16, 300)];
    let mut speeds = Vec::new();
    for b in &mats {
        let mut t = line(700, std::slice::from_ref(b), |_| 0);
        let c = t.sound_speed(1);
        packet(&mut t, 150.0, 16.0, 2f64.powi(24), 1.0);
        let ticks = (300.0 / c) as usize;
        for _ in 0..ticks / 5 {
            t.step();
        }
        let x1 = centroid(&t, 0..700);
        for _ in ticks / 5..ticks {
            t.step();
        }
        let x2 = centroid(&t, 0..700);
        let measured = (x2 - x1) / (ticks - ticks / 5) as f64;
        println!(
            "len {:2} k {} : predicted c = sqrt(k·w) = {c:.5}, measured {measured:.5} cells/tick ({:+.2}%)",
            b.len(),
            t.stiffness(1, 1),
            100.0 * (measured / c - 1.0)
        );
        assert!((measured / c - 1.0).abs() < 0.01);
        speeds.push(measured);
    }
    // Light, stiff matter rings fast: the ratio of the extremes is the ratio of sqrt(k/m).
    println!("fastest / slowest = {:.3}", speeds[0] / speeds[2]);
    assert!(speeds[0] > 3.0 * speeds[2]);
}

#[test]
fn impedance_mismatch_reflects_as_predicted() {
    let a = springy(1, 2, 400);
    let b = find(31, 2, 16, 1_000_000, |b| bond(b, b) >= 300 && bond(&a, b) >= 150);
    let c = find(32, 2, 2, 1_000_000, |c| bond(c, c) >= 300 && bond(&a, c) <= 0);
    let half = 450u32;
    // A light line meets a heavy one.
    let mut t = line(900, &[a.clone(), b.clone()], |x| usize::from(x >= half));
    packet(&mut t, 200.0, 16.0, 2f64.powi(24), 1.0);
    let e0 = t.energy().total;
    for _ in 0..(400.0 / t.sound_speed(1)) as usize {
        t.step();
    }
    let back: f64 = (0..half as usize - 1).map(|x| t.cell_energy(x)).sum();
    let (ka, kb) = (f64::from(t.stiffness(1, 1)), f64::from(t.stiffness(2, 2)));
    let (wa, wb) = (f64::from(t.inverse_mass(1)), f64::from(t.inverse_mass(2)));
    let (za, zb) = ((ka / wa).sqrt(), (kb / wb).sqrt());
    let predicted = ((za - zb) / (za + zb)).powi(2);
    let measured = back / e0;
    println!(
        "Z ratio {:.3}, interface bond {}: reflected {measured:.4} of the energy, predicted {predicted:.4}",
        zb / za,
        bond(&a, &b)
    );
    assert!((measured - predicted).abs() < 0.02);
    // An incompatible face (bond ≤ 0) carries no spring: everything comes back, and the far side
    // never moves by a single unit.
    let mut t = line(900, &[a.clone(), c.clone()], |x| usize::from(x >= half));
    packet(&mut t, 200.0, 16.0, 2f64.powi(24), 1.0);
    let e0 = t.energy().total;
    for _ in 0..(400.0 / t.sound_speed(1)) as usize {
        t.step();
        assert!((half as usize..900).all(|x| t.u[x] == 0 && t.p[x] == 0));
    }
    let back: f64 = (0..half as usize).map(|x| t.cell_energy(x)).sum();
    println!("incompatible face (bond {}): reflected {:.6}", bond(&a, &c), back / e0);
    assert!((back / e0 - 1.0).abs() < 0.001);
}

#[test]
fn resonator_rings_forever() {
    // A cube of springy matter in void: an isolated body. Struck once, it rings with no loss.
    let a = springy(1, 2, 400);
    let mut m = Matter::new(Grid::new(7, 7, 7));
    let ia = m.intern(&a);
    m.fill((2, 2, 2), (4, 4, 4), ia);
    let mut t = Toda::new(m);
    let g = t.matter().grid;
    let corner = g.idx(2, 2, 2);
    t.poke(corner, 5_000_000);
    let e0 = t.energy().shadow;
    let ticks = 200_000;
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let (mut early, mut late) = (0i64, 0i64);
    let mut kin_late = 0.0;
    for k in 0..ticks {
        t.step();
        let swing = i64::from(t.u[corner].wrapping_sub(t.u[g.idx(4, 4, 4)])).abs();
        if k < 2000 {
            early = early.max(swing);
        }
        if k >= ticks - 2000 {
            late = late.max(swing);
            kin_late += t.energy().kinetic / t.energy().total / 2000.0;
        }
        if k % 1000 == 0 {
            let e = t.energy().shadow;
            lo = lo.min(e);
            hi = hi.max(e);
        }
    }
    println!(
        "after {ticks} ticks: shadow energy band {:.5}%, corner swing early {early} late {late}, kinetic share {kin_late:.3}",
        100.0 * (hi - lo) / e0
    );
    println!("{}", slice(&t, 3));
    assert!((hi - lo) / e0 < 0.002);
    assert!(late as f64 > 0.3 * early as f64, "still ringing");
    assert!((kin_late - 0.5).abs() < 0.15, "virial: half kinetic, half potential");
}

#[test]
fn a_dimer_is_an_exact_clock() {
    // Two blocks on one spring. The integer map is a bijection, and a bounded orbit of a bijection
    // on a finite set is a pure cycle: the state returns bit for bit (Poincaré recurrence, exact).
    let a = springy(1, 2, 400);
    let mut t = line(2, std::slice::from_ref(&a), |_| 0);
    t.poke(0, 1_000_000);
    t.poke(1, -1_000_000);
    let (u0, p0) = (t.u.clone(), t.p.clone());
    let w = 2.0 * f64::from(t.stiffness(1, 1)) / 2f64.powi(t.shift() as i32) * f64::from(t.inverse_mass(1))
        / 2f64.powi(MASS_SHIFT as i32);
    let linear = 2.0 * std::f64::consts::PI / (2.0 * (w.sqrt() / 2.0).asin());
    let mut crossings = Vec::new();
    let mut last = t.p[0];
    let mut period = 0u64;
    for k in 1..=3_000_000u64 {
        t.step();
        if (t.p[0] > 0) != (last > 0) && t.p[0] > 0 {
            crossings.push(k);
        }
        last = t.p[0];
        if t.u == u0 && t.p == p0 {
            period = k;
            break;
        }
    }
    let mean = (crossings[crossings.len() - 1] - crossings[0]) as f64 / (crossings.len() - 1) as f64;
    println!(
        "dimer: oscillation period {mean:.3} ticks (leapfrog theory {linear:.3}); the whole state recurs exactly after {period} ticks"
    );
    assert!(period > 0, "an exact recurrence within three million ticks");
    assert!((mean / linear - 1.0).abs() < 0.01);
}

#[test]
fn soliton_keeps_its_shape_where_linear_waves_spread() {
    let a = springy(1, 2, 400);
    let mut rows = Vec::new();
    for (name, p0) in [("soliton", 10_000_000), ("linear", 100_000)] {
        let mut t = line(1600, std::slice::from_ref(&a), |_| 0);
        let c = t.sound_speed(1);
        t.poke(0, p0);
        let mut picture = String::new();
        let mut seen = Vec::new();
        let mut top = 0.0;
        for k in 1..=8000 {
            t.step();
            if k % 500 == 0 {
                let s = strain(&t);
                if top == 0.0 {
                    top = s.iter().fold(0.0f64, |m, v| m.max(v.abs()));
                }
                picture.push_str(&format!("{k:5} |{}|\n", ribbon(&s, 100, top)));
            }
            if k == 2000 || k == 8000 {
                seen.push((k, lead(&strain(&t), 0.3)));
            }
        }
        let [(k1, (x1, peak1, w1)), (k2, (x2, peak2, w2))] = [seen[0], seen[1]];
        let speed = (x2 - x1) as f64 / (k2 - k1) as f64;
        println!("{name}: kick {p0}");
        print!("{picture}");
        println!(
            "{name}: peak strain {:.5} → {:.5} of half a period (×{:.3}), width {w1} → {w2} cells, speed {speed:.4} vs sound {c:.4} ({:+.1}%)",
            peak1 / 2f64.powi(31),
            peak2 / 2f64.powi(31),
            peak2 / peak1,
            100.0 * (speed / c - 1.0)
        );
        rows.push((peak2 / peak1, w1, w2, speed / c));
    }
    let (sol, lin) = (rows[0], rows[1]);
    assert!(sol.0 > 0.9 && sol.2.abs_diff(sol.1) <= 1, "the soliton holds its height and width");
    assert!(sol.3 > 1.01, "the soliton is supersonic");
    assert!(lin.0 < 0.8 && lin.2 as f64 >= 1.4 * lin.1 as f64, "the linear pulse disperses");
}

#[test]
fn fput_recurrence_then_thermalisation() {
    // Fermi, Pasta, Ulam and Tsingou's experiment on a free line of 32 blocks: all energy in the
    // lowest mode. Moderately nonlinear, it wanders into a few modes and comes back; strongly
    // nonlinear, it spreads over all of them.
    let a = springy(1, 2, 400);
    let n = 32u32;
    let mut results = Vec::new();
    for tmax in [0.1, 0.3] {
        let mut t = line(n, std::slice::from_ref(&a), |_| 0);
        let amp = tmax * 2f64.powi(31) * f64::from(n) / std::f64::consts::PI;
        for x in 0..n as usize {
            let u = amp * (std::f64::consts::PI * (x as f64 + 0.5) / f64::from(n)).cos();
            t.set(x, u as i64 as i32, 0);
        }
        let (mut min1, mut low_at, mut back_at) = (1.0f64, 0, 0);
        let (mut neff, mut samples) = (0.0, 0.0);
        let mut trace = Vec::new();
        let ticks = 400_000;
        for k in 0..ticks {
            if k % 500 == 0 {
                let e = mode_energies(&t, 1);
                let total: f64 = e[1..].iter().sum();
                let f1 = e[1] / total;
                if f1 < min1 {
                    min1 = f1;
                }
                if low_at == 0 && f1 < 0.3 {
                    low_at = k;
                }
                if low_at > 0 && back_at == 0 && f1 > 0.95 {
                    back_at = k;
                }
                if k >= ticks / 2 {
                    let s: f64 = e[1..].iter().map(|&x| x / total).filter(|&x| x > 0.0).map(|x| -x * x.ln()).sum();
                    neff += s.exp() / f64::from(n - 1);
                    samples += 1.0;
                }
                if k % 20_000 == 0 {
                    trace.push(format!("{f1:.2}"));
                }
            }
            t.step();
        }
        neff /= samples;
        println!("strain {tmax}: mode-1 share {}", trace.join(" "));
        println!(
            "strain {tmax}: lowest share {min1:.3} (below 0.3 at tick {low_at}), back above 0.95 at tick {back_at}, late effective modes {:.3} of 31",
            neff * 31.0
        );
        results.push((min1, back_at, neff));
    }
    let (fpu, hot) = (results[0], results[1]);
    assert!(fpu.0 < 0.3 && fpu.1 > 0, "energy leaves the first mode and comes back");
    assert!(fpu.2 < 0.2, "and stays in a few modes");
    assert!(hot.2 > 2.0 * fpu.2 && hot.1 == 0, "strong nonlinearity spreads it and it does not return");
}

/// Sub-cell position of the strain peak inside `range`: the |strain|²-weighted mean of the seven
/// cells around the largest.
fn peak_at(s: &[f64], range: std::ops::Range<usize>) -> (f64, f64) {
    let at = range.clone().max_by(|&a, &b| s[a].abs().total_cmp(&s[b].abs())).unwrap_or(range.start);
    let (mut sx, mut sw) = (0.0, 0.0);
    let lo = at.saturating_sub(3);
    for (x, v) in s.iter().enumerate().take(at + 4).skip(lo) {
        sx += v * v * x as f64;
        sw += v * v;
    }
    (sx / sw, s[at].abs())
}

#[test]
fn solitons_collide_elastically() {
    // Two solitons meet head-on in the middle of a line. In the linear chain they would just
    // superpose; here they interact, yet each leaves with its height, shifted by a fraction of a
    // cell. Elastic collisions are what makes a soliton a particle, and also why they compute
    // nothing on their own.
    let a = springy(1, 2, 400);
    let n = 1200u32;
    let mut runs = Vec::new();
    for both in [false, true] {
        let mut t = line(n, std::slice::from_ref(&a), |_| 0);
        t.poke(0, 10_000_000);
        if both {
            t.poke(n as usize - 1, -10_000_000);
        }
        let mut during = 0.0f64;
        for k in 0..5000 {
            t.step();
            if (3000..3600).contains(&k) {
                during = during.max(strain(&t).iter().fold(0.0f64, |m, v| m.max(v.abs())));
            }
        }
        let (x, h) = peak_at(&strain(&t), 700..n as usize - 1);
        runs.push((x, h, during));
    }
    let (solo, pair) = (runs[0], runs[1]);
    let half = 2f64.powi(31);
    println!(
        "alone: peak at {:.2} height {:.4}; after the collision: at {:.2} height {:.4} (shift {:+.2} cells); strain at the crossing {:.4} vs alone {:.4}",
        solo.0,
        solo.1 / half,
        pair.0,
        pair.1 / half,
        pair.0 - solo.0,
        pair.2 / half,
        solo.2 / half
    );
    assert!((pair.1 / solo.1 - 1.0).abs() < 0.05, "heights survive");
    assert!((pair.0 - solo.0).abs() < 2.0, "a phase shift of under two cells");
    assert!(pair.2 > 1.8 * solo.2, "they did meet");
}

#[test]
fn junction_logic_attempt() {
    // Two input lines meet at a Y and feed one output line through a weakly bonded link block. A
    // weak bond is a fuse: past half a period of strain it slips. Gentle pulses add (the link
    // passes both, coherently); hard pulses arriving together slip the fuse and less gets
    // through than from one alone. That is an XOR-like response, measured in energy.
    let a = springy(1, 2, 400);
    let link = find(21, 2, 2, 1_000_000, |b| (40..=70).contains(&bond(&a, b)) && bond(b, b) > 0);
    let mut tables = Vec::new();
    for p0 in [6_000_000, 20_000_000] {
        let mut row = Vec::new();
        for (ia, ib) in [(1, 0), (0, 1), (1, 1)] {
            let mut t = junction(&a, &link, 300, 300);
            let g = t.matter().grid;
            if ia == 1 {
                t.poke(g.idx(0, 0, 0), p0);
            }
            if ib == 1 {
                t.poke(g.idx(0, 2, 0), p0);
            }
            let e0 = t.energy().total;
            for _ in 0..3000 {
                t.step();
            }
            let out: f64 = (303..g.nx).map(|x| t.cell_energy(g.idx(x, 1, 0))).sum();
            row.push((out, out / e0));
        }
        println!(
            "kick {p0}, link bond {}: out(1,0) {:.3e} out(0,1) {:.3e} out(1,1) {:.3e}  →  (1,1)/(1,0) = {:.2}; best output share {:.3}",
            bond(&a, &link),
            row[0].0,
            row[1].0,
            row[2].0,
            row[2].0 / row[0].0,
            row.iter().fold(0.0f64, |m, r| m.max(r.1))
        );
        tables.push(row);
    }
    let (soft, hard) = (&tables[0], &tables[1]);
    assert_eq!(soft[0].0, soft[1].0, "the junction is symmetric");
    assert!(soft[2].0 > 3.5 * soft[0].0, "gentle pulses superpose (≈4× for coherent addition)");
    assert!(hard[2].0 < hard[0].0, "hard pulses together get through less than one alone");
    // No gain: the output never carries more than a fraction of what went in.
    assert!(tables.iter().flatten().all(|r| r.1 < 0.3));
}

#[test]
fn a_hard_hit_slips_a_bond_and_it_heals() {
    // Past the spring's peak the bond softens and gives way. Hit hard enough, the end block slips
    // whole periods against its neighbour (fracture), spinning for a while; each slip lands it in
    // the next well of the periodic bond, which is the same bond again. So the break heals by
    // itself and the spin radiates away as sound: no lasting rotor, nothing lost.
    let a = springy(1, 2, 400);
    for p0 in [10_000_000, 30_000_000] {
        let mut t = line(64, std::slice::from_ref(&a), |_| 0);
        t.poke(0, p0);
        let e0 = t.energy().shadow;
        let (mut slips, mut last, mut last_slip) = (0, 0i32, 0);
        let mut late_share = 0.0;
        for k in 0..20_000 {
            t.step();
            let d = t.u[1].wrapping_sub(t.u[0]);
            if (d > 0) != (last > 0) && d.unsigned_abs() > 1 << 30 {
                slips += 1;
                last_slip = k;
            }
            last = d;
            if k >= 19_000 {
                late_share += t.cell_energy(0) / t.energy().total / 1000.0;
            }
        }
        let drift = (t.energy().shadow - e0) / e0;
        println!(
            "kick {p0}: the end bond slipped {slips} times, the last at tick {last_slip}; the end block holds {late_share:.3} of the energy late; shadow drift {drift:+.5}"
        );
        if p0 == 10_000_000 {
            assert_eq!(slips, 0);
        } else {
            assert!(slips > 10, "the bond breaks and the block spins");
            assert!(last_slip < 10_000, "then it re-binds");
        }
        assert!(late_share < 0.1, "and the energy leaves as sound");
        assert!(drift.abs() < 0.01);
    }
}

#[test]
fn heat_is_only_statistical() {
    // One blow spreads through the slab into an incoherent buzz (the participation ratio, the
    // share of cells that hold the energy, climbs), the coarse-grained arrow of time. Then rewind:
    // the buzz reassembles into the blow exactly, because nothing was ever lost.
    let mut t = slab(16, 16, 16);
    let g = t.matter().grid;
    let cells = matter_cells(&t);
    let share = |t: &Toda| {
        let e: Vec<f64> = cells.iter().map(|&i| t.cell_energy(i)).collect();
        let s: f64 = e.iter().sum();
        let s2: f64 = e.iter().map(|x| x * x).sum();
        s * s / s2 / cells.len() as f64
    };
    t.poke(g.idx(8, 6, 8), 5_000_000);
    let d0 = t.digest();
    let r0 = share(&t);
    let mut trace = vec![format!("{r0:.4}")];
    for k in 1..=20_000 {
        t.step();
        if k % 4000 == 0 {
            trace.push(format!("{:.4}", share(&t)));
        }
    }
    let r1 = share(&t);
    for _ in 0..20_000 {
        t.unstep();
    }
    println!("participation ratio every 4000 ticks: {} ; rewound: {:.4}", trace.join(" → "), share(&t));
    assert!(r1 > 100.0 * r0);
    assert_eq!(t.digest(), d0);
}

#[test]
fn rewind_undoes_play() {
    // The gameplay verb: poke, watch, rewind through the lab's `unstep`, and unpoke.
    let pristine = slab(12, 12, 12);
    let mut c: Box<dyn Candidate> = Box::new(pristine.clone());
    let g = c.matter().grid;
    let d0 = c.digest();
    let at = g.idx(5, 4, 6);
    c.poke(at, 8_000_000);
    for _ in 0..1500 {
        c.step();
    }
    println!("after a poke and 1500 ticks: {} cells working, ledger {:?}", c.active(), c.ledger());
    assert_ne!(c.digest(), d0);
    for _ in 0..1500 {
        assert!(c.unstep());
    }
    c.poke(at, -8_000_000);
    assert_eq!(c.digest(), d0);
    assert_eq!(c.ledger().get("momentum"), Some(0));
}

#[test]
fn rest_costs_nothing_and_unbonded_matter_is_deaf() {
    let mut t = slab(16, 16, 16);
    let g = t.matter().grid;
    for _ in 0..100 {
        t.step();
    }
    assert_eq!((t.active(), t.hot()), (0, 0), "a world at rest does no work");
    // A poke in springy matter wakes a growing neighbourhood.
    let mut s = t.clone();
    s.poke(g.idx(8, 6, 8), 2_000_000);
    let mut grow = Vec::new();
    for k in 1..=400 {
        s.step();
        if k % 100 == 0 {
            grow.push(s.active());
        }
    }
    println!("cells worked at ticks 100..400 after one poke: {grow:?} of {}", g.len());
    assert!(grow.windows(2).all(|w| w[0] <= w[1]) && grow[3] > 100);
    // A block of pure matter bonds 0 to its own kind: inside the pure layer it hangs on no spring,
    // so a poke sends it drifting forever, alone (Newton's first law), and nobody hears it.
    let lone = g.idx(1, 0, 8);
    assert_eq!(t.links[lone], 0);
    t.poke(lone, 1_000_000);
    for _ in 0..1000 {
        t.step();
    }
    let moved: Vec<usize> = (0..g.len()).filter(|&i| t.u[i] != 0 || t.p[i] != 0).collect();
    println!("pure block poked: after 1000 ticks its phase is {} (it wrapped the torus); cells disturbed {:?}", t.u[lone], moved);
    assert_eq!(moved, vec![lone]);
    assert_eq!(t.active(), 1);
}
