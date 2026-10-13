//! The experiments. Every material comes from a deterministic search over the fit readings; every
//! assertion is a measured phenomenon. Run with `--nocapture` to see the views and numbers.

use pwc_law_core::search::{bond, find, is_cohesive};
use pwc_law_core::{show, Candidate, Grid, Mat, Matter, VOID};

use crate::material::{probe_response, Block, Law};
use crate::{eps_inv, Polyakov, VACUUM, WRAP};

fn light(b: &Block) -> i32 {
    probe_response(b, Law::current().probes.light)
}

fn glow(b: &Block) -> i32 {
    probe_response(b, Law::current().probes.glow)
}

/// A cohesive conductor: full carrier mobility, vacuum permittivity.
fn conductor() -> Block {
    find(1, 3, 16, 200_000, |b| is_cohesive(b) && glow(b) >= 256 && light(b) <= 0)
}

/// A cohesive poor conductor: it discharges a quarter or so of a face's flux per tick.
fn resistor() -> Block {
    find(4, 3, 16, 200_000, |b| is_cohesive(b) && (48..=96).contains(&glow(b)) && light(b) <= 0)
}

/// A cohesive insulator that slows the field: no carriers, `ε ≥ 3`.
fn dielectric() -> Block {
    find(2, 3, 16, 200_000, |b| is_cohesive(b) && glow(b) <= 0 && light(b) >= 512)
}

/// A second conductor that is incompatible with `a`: their shared face has a breakdown field.
fn rival(a: &Block, barrier: i32) -> Block {
    find(3, 3, 16, 400_000, |b| is_cohesive(b) && glow(b) >= 256 && light(b) <= 0 && bond(a, b) <= -barrier)
}

/// Gauss's law and `div B = 0` as integers, and total charge 0, asserted for the current state.
fn assert_exact(c: &Polyakov, tick: usize) {
    assert_eq!(c.gauss_defects(), (0, 0), "Gauss's law broke at tick {tick}");
    assert_eq!(c.monopoles(), (0, 0, 0), "div B broke at tick {tick}");
    assert_eq!(c.total_charge(), 0, "charge changed at tick {tick}");
}

/// One row of a candidate, `x` in `lo..hi`, in the ramp of `pwc_law_core::show`.
fn row_view(c: &Polyakov, y: u32, z: u32, lo: u32, hi: u32) -> String {
    const RAMP: &[u8] = b".:-=+*#%@";
    let g = c.matter().grid;
    (lo..hi)
        .map(|x| {
            let i = g.idx(x, y, z);
            match c.show(i) {
                0 if c.matter().get(i) == VOID => ' ',
                0 => '\u{b7}',
                v => RAMP[((v as usize - 1) * RAMP.len()) / 255] as char,
            }
        })
        .collect()
}

/// A mixed box: a conductor slab, a dielectric block, two incompatible conductors touching, void.
fn mixed_box() -> Polyakov {
    let (a, d) = (conductor(), dielectric());
    let r = rival(&a, 300);
    let mut m = Matter::new(Grid::new(16, 14, 12));
    let (ia, id, ir) = (m.intern(&a), m.intern(&d), m.intern(&r));
    m.fill((0, 0, 0), (15, 13, 2), ia);
    m.fill((3, 3, 3), (8, 9, 7), id);
    m.fill((10, 2, 3), (12, 11, 6), ir);
    m.fill((10, 2, 7), (12, 11, 8), ia);
    Polyakov::new(m)
}

#[test]
fn gauss_law_and_div_b_hold_exactly_every_tick() {
    let mut c = mixed_box();
    let g = c.matter().grid;
    // Pairs in void, in the dielectric, inside a conductor, across the incompatible junction; kicks.
    let pokes = [(2, 12, 10, 3000), (5, 5, 5, -2500), (7, 7, 1, 4000), (11, 6, 6, 6000), (11, 6, 7, -6000)];
    for &(x, y, z, s) in &pokes {
        c.poke(g.idx(x, y, z), s);
    }
    c.kick(g.idx(8, 11, 9), 2, 3000);
    c.kick(g.idx(4, 4, 10), 0, -2000);
    assert_exact(&c, 0);
    let mut energy = (i64::MAX, 0);
    for t in 1..=3000 {
        c.step();
        if t % 500 == 250 {
            c.poke(g.idx((t % 13) as u32 + 1, 7, 9), 2000);
        }
        assert_exact(&c, t);
        let u = c.energy();
        energy = (energy.0.min(u), energy.1.max(u));
    }
    println!("{}", show::slice(&c, 5));
    println!("3000 ticks, 13 pokes and kicks: Gauss and div B exact in every cell every tick");
    let mut twin = mixed_box();
    for &(x, y, z, s) in &pokes {
        twin.poke(g.idx(x, y, z), s);
    }
    twin.kick(g.idx(8, 11, 9), 2, 3000);
    twin.kick(g.idx(4, 4, 10), 0, -2000);
    for t in 1..=3000 {
        twin.step();
        if t % 500 == 250 {
            twin.poke(g.idx((t % 13) as u32 + 1, 7, 9), 2000);
        }
    }
    println!("twin run digest {:016x} = {:016x}", twin.digest(), c.digest());
    assert_eq!(twin.digest(), c.digest(), "two peers diverged");
    println!("field energy range {energy:?}, Joule loss {}", c.joule());
    assert!(c.joule() > 0, "the conductors dissipated nothing");
}

#[test]
fn ampere_is_exactly_the_transpose_of_faraday_and_the_box_surface_holds_nothing() {
    // Unit E on one face through Faraday gives a column of curl; unit H on one edge through Ampère
    // gives a column of curlᵀ. They must be each other's transpose, entry by entry.
    let g = Grid::new(4, 3, 3);
    let n = g.len();
    let unit = |field: &mut Polyakov, k: usize, i: usize| {
        for v in &mut field.e {
            v.fill(0);
        }
        field.e[k][i] = 1;
        field.wake_all();
    };
    // Inputs live only where the law ever puts values: faces and edges with all their cells.
    let face_exists = |k: usize, i: usize| [g.pos(i).0, g.pos(i).1, g.pos(i).2][k] >= 1;
    let columns: Vec<_> = (0..3 * n)
        .map(|f| {
            let mut c = Polyakov::new(Matter::new(g));
            if face_exists(f / n, f % n) {
                unit(&mut c, f / n, f % n);
                c.faraday(-1);
            }
            c.b
        })
        .collect();
    let (mut entries, mut nonzero) = (0, 0);
    for edge in (0..3 * n).filter(|&e| c_edge_exists(g, e % n, e / n)) {
        let mut c = Polyakov::new(Matter::new(g));
        unit(&mut c, edge / n, edge % n);
        c.ampere(1);
        for (f, column) in columns.iter().enumerate() {
            let (k, i) = (f / n, f % n);
            let curl = -i32::from(column[edge / n][edge % n]);
            let curl_t = i32::from(c.d[k][i]);
            assert_eq!(curl, curl_t, "face ({i}, {k}) and edge ({}, {})", edge % n, edge / n);
            if !face_exists(k, i) {
                assert_eq!(curl_t, 0, "a face on the box surface received flux");
            }
            entries += 1;
            nonzero += usize::from(curl != 0);
        }
    }
    for col in &columns {
        for (k, v) in col.iter().enumerate() {
            for (i, &x) in v.iter().enumerate() {
                if !c_edge_exists(g, i, k) {
                    assert_eq!(x, 0, "an edge without four cells received flux");
                }
            }
        }
    }
    println!("curl on a 4×3×3 box: {entries} entries compared, {nonzero} nonzero, all equal to curlᵀ");
}

fn c_edge_exists(g: Grid, i: usize, axis: usize) -> bool {
    let (x, y, z) = g.pos(i);
    let at = [x, y, z];
    (0..3).filter(|&k| k != axis).all(|k| at[k] >= 1)
}

#[test]
fn the_field_runs_backwards_exactly_without_currents() {
    let d = dielectric();
    let mut m = Matter::new(Grid::new(20, 18, 16));
    let id = m.intern(&d);
    m.fill((4, 4, 4), (12, 10, 9), id);
    let mut c = Polyakov::new(m);
    let g = c.matter().grid;
    c.poke(g.idx(6, 6, 6), 5000);
    c.poke(g.idx(15, 3, 12), -7000);
    c.kick(g.idx(10, 10, 10), 1, 9000);
    let start = c.digest();
    for _ in 0..2000 {
        c.step();
    }
    let mid = c.digest();
    println!("after 2000 ticks (z = 6):\n{}", show::slice(&c, 6));
    for _ in 0..2000 {
        assert!(c.unstep());
    }
    println!("after 2000 ticks back (z = 6): the two pairs and the kick again\n{}", show::slice(&c, 6));
    println!("digest start {start:016x}, after 2000 ticks {mid:016x}, after 2000 back {:016x}", c.digest());
    assert_ne!(start, mid);
    assert_eq!(c.digest(), start, "the field part is not a bijection");
}

#[test]
fn ohm_forgets_so_conductors_break_reversal() {
    let a = conductor();
    let mut m = Matter::new(Grid::new(12, 12, 12));
    let ia = m.intern(&a);
    m.fill((3, 3, 3), (8, 8, 8), ia);
    let mut c = Polyakov::new(m);
    let g = c.matter().grid;
    c.poke(g.idx(5, 5, 5), 4000);
    let start = c.digest();
    for _ in 0..200 {
        c.step();
    }
    assert!(!c.unstep(), "a box with conductors refuses to promise an exact rewind");
    for _ in 0..200 {
        c.step_back();
    }
    println!("conductor box: Joule loss {}, digest after forward+back {:016x} vs start {start:016x}", c.joule(), c.digest());
    assert!(c.joule() > 0);
    assert_ne!(c.digest(), start, "a current should have been irreversible");
}

/// Position of the largest |B_z| on row `y` among `x` in `lo..hi`, refined by a parabola.
fn peak(c: &Polyakov, y: u32, lo: u32, hi: u32) -> (f64, i32) {
    let g = c.matter().grid;
    let v = |x: u32| i32::from(c.magnetic(g.idx(x, y, 0), 2));
    let mut best = lo;
    for x in lo..hi {
        if v(x).abs() > v(best).abs() {
            best = x;
        }
    }
    let (l, m, r) = (f64::from(v(best - 1)), f64::from(v(best)), f64::from(v(best + 1)));
    let den = l - 2.0 * m + r;
    let off = if den != 0.0 { 0.5 * (l - r) / den } else { 0.0 };
    (f64::from(best) + off, v(best))
}

/// A 2-D sheet (`nz = 1`: fields `D_x`, `D_y`, `B_z`) with a smooth `B_z` bump across all rows.
fn sheet(nx: u32, ny: u32, fill_from: Option<(u32, Mat, Matter)>) -> Polyakov {
    let m = match fill_from {
        Some((x0, mat, mut m)) => {
            m.fill((x0 as i32, 0, 0), (nx as i32 - 1, ny as i32 - 1, 0), mat);
            m
        }
        None => Matter::new(Grid::new(nx, ny, 1)),
    };
    Polyakov::new(m)
}

/// Field energy (as [`Polyakov::energy`]) of the cells with x in `lo..hi` in the middle 80 rows, which
/// the disturbance from the walls (a plane wave is not a mode of a closed box) has not reached yet.
fn energy_in(c: &Polyakov, lo: u32, hi: u32) -> i64 {
    let g = c.matter().grid;
    let mut u = 0i64;
    for i in 0..g.len() {
        let (x, y, z) = g.pos(i);
        if x < lo || x >= hi || y + 40 < g.ny / 2 || y >= g.ny / 2 + 40 {
            continue;
        }
        for (k, has) in [(0, x > 0), (1, y > 0), (2, z > 0)] {
            if has {
                let dv = i64::from(c.flux(i, k));
                u += i64::from(c.eps_face(i, k)) * dv * dv;
            }
            let bv = i64::from(c.magnetic(i, k));
            u += i64::from(VACUUM) * bv * bv;
        }
    }
    u / 256
}

fn launch(c: &mut Polyakov, x0: u32, half: u32, amp: i32) {
    let g = c.matter().grid;
    for y in 1..g.ny {
        for dx in 0..=2 * half {
            let x = x0 + dx - half;
            let w = half as i32 + 1 - (dx as i32 - half as i32).abs();
            c.b[2][g.idx(x, y, 0)] = (amp * w / (half as i32 + 1)) as i16;
        }
        c.mark(g.idx(0, y, 0), crate::LB);
    }
}

#[test]
fn waves_travel_at_half_a_cell_per_tick_and_slow_by_root_epsilon() {
    // Vacuum: the right-moving half of a bump, tracked on the middle row, away from every wall.
    let (nx, ny) = (400, 480);
    let mid = ny / 2;
    let mut c = sheet(nx, ny, None);
    launch(&mut c, 130, 16, 8000);
    let mut track = Vec::new();
    for t in 0..=150 {
        if t % 50 == 0 {
            track.push((t, peak(&c, mid, 130 + t / 2 - 10, 130 + t / 2 + 30)));
        }
        c.step();
    }
    let v_vac = (track[3].1.0 - track[1].1.0) / 100.0;
    println!("vacuum peak track (tick, (x, B_z)) {track:?}");
    println!("vacuum speed {v_vac:.4} cells/tick (continuum c = sqrt(1/2 * 1/2) = 0.5)");
    assert!((v_vac - 0.5).abs() < 0.01, "vacuum speed {v_vac}");

    // A dielectric half-space from x = 210: speed inside, Fresnel reflection and transmission.
    let d = dielectric();
    let mut m = Matter::new(Grid::new(nx, ny, 1));
    let id = m.intern(&d);
    let e = eps_inv(m.reading(id));
    let n = (f64::from(VACUUM) / f64::from(e)).sqrt();
    let mut c = sheet(nx, ny, Some((210, id, m)));
    launch(&mut c, 130, 16, 8000);
    let mut incident = (0, 0);
    let (mut inside, mut reflected, mut energies) = (Vec::new(), (0.0, 0), (0, 0));
    for t in 0..=330 {
        if t % 60 == 0 {
            println!("t {t:>3} |{}|", row_view(&c, mid, 0, 100, 300));
        }
        if t == 100 {
            incident = (peak(&c, mid, 165, 200).1, energy_in(&c, 140, 210));
        }
        if (240..=330).contains(&t) && t % 30 == 0 {
            inside.push((t, peak(&c, mid, 212, nx - 2)));
        }
        if t == 330 {
            reflected = peak(&c, mid, 90, 200);
            energies = (energy_in(&c, 210, nx), energy_in(&c, 90, 210));
        }
        c.step();
    }
    let v_in = (inside[3].1.0 - inside[0].1.0) / 90.0;
    let transmitted = inside[0].1.1;
    let (r, tr) = (f64::from(reflected.1) / f64::from(incident.0), f64::from(transmitted) / f64::from(incident.0));
    let (r0, t0) = ((n - 1.0) / (n + 1.0), 2.0 * n / (n + 1.0));
    let (big_t, big_r) = (energies.0 as f64 / incident.1 as f64, energies.1 as f64 / incident.1 as f64);
    println!("dielectric: light {}, eps_inv {e}/256 (vacuum {VACUUM}), index n = {n:.3}", light(&d));
    println!("inside track {inside:?}, reflected peak {reflected:?}");
    println!("speed inside {v_in:.4} (predicted {:.4}); index measured {:.3}", 0.5 / n, v_vac / v_in);
    println!("B reflected/incident {r:.3} (Fresnel {r0:.3}), transmitted/incident {tr:.3} (Fresnel {t0:.3})");
    assert!((v_vac / v_in - n).abs() / n < 0.03, "index {}", v_vac / v_in);
    assert!((r - r0).abs() < 0.03, "reflection {r}");
    println!("energy transmitted {big_t:.3} (Fresnel {:.3}), reflected {big_r:.3} (Fresnel {:.3})", 1.0 - r0 * r0, r0 * r0);
    assert!((big_t - (1.0 - r0 * r0)).abs() < 0.02, "energy transmission {big_t}");
    assert!((big_r - r0 * r0).abs() < 0.02, "energy reflection {big_r}");
}

/// A Wilson line: `+s` at `from`, `−s` where `n` steps along `dir` end, flux along the path.
fn wilson(c: &mut Polyakov, from: usize, dir: usize, n: usize, s: i16) -> usize {
    let mut i = from;
    for _ in 0..n {
        assert!(c.pair(i, dir, s));
        i = c.matter().grid.step(i, dir).expect("inside");
    }
    i
}

/// Sum of `f(cell)` over an inclusive box of positions.
fn sum_box(c: &Polyakov, lo: (u32, u32, u32), hi: (u32, u32, u32), f: impl Fn(usize) -> i64) -> i64 {
    let g = c.matter().grid;
    let mut s = 0;
    for z in lo.2..=hi.2 {
        for y in lo.1..=hi.1 {
            for x in lo.0..=hi.0 {
                s += f(g.idx(x, y, z));
            }
        }
    }
    s
}

/// `E` across cell `i`'s −axis face, as the law computes it.
fn field(c: &Polyakov, i: usize, axis: usize) -> i64 {
    let v = i64::from(c.flux(i, axis)) * i64::from(c.eps_face(i, axis));
    (v.abs() + 127) / 256 * v.signum()
}

/// Parallel plates 16×16 cells, a 4-cell gap (`gap` fills it), charged by 64 Wilson lines.
/// Returns (charge on plate A, mean voltage across the gap centre, its spread) over the last 1000 of
/// 3000 ticks, asserting the plate's charge never changes.
fn capacitor(gap: Option<&Block>) -> (i64, f64, f64, Polyakov) {
    let a = resistor();
    let mut m = Matter::new(Grid::new(26, 22, 22));
    let ia = m.intern(&a);
    m.fill((8, 3, 3), (9, 18, 18), ia);
    m.fill((14, 3, 3), (15, 18, 18), ia);
    if let Some(d) = gap {
        let id = m.intern(d);
        m.fill((10, 3, 3), (13, 18, 18), id);
    }
    let mut c = Polyakov::new(m);
    let g = c.matter().grid;
    for z in 7..=14 {
        for y in 7..=14 {
            wilson(&mut c, g.idx(9, y, z), 3, 5, 400);
        }
    }
    let plate = |c: &Polyakov| sum_box(c, (8, 3, 3), (9, 18, 18), |i| i64::from(c.charge(i)));
    let q = plate(&c);
    let mut v = Vec::new();
    for t in 0..3000 {
        c.step();
        assert_eq!(plate(&c), q, "charge crossed the gap at tick {t}");
        if t >= 2000 {
            v.push((10..=14).map(|x| field(&c, g.idx(x, 11, 11), 0)).sum::<i64>() as f64);
        }
    }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let spread = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
    (q, mean, spread, c)
}

#[test]
fn a_capacitor_holds_its_charge_and_a_dielectric_raises_its_capacitance() {
    let (q, v_vac, s_vac, c) = capacitor(None);
    println!("{}", show::slice(&c, 11));
    let g = c.matter().grid;
    let inside = field(&c, g.idx(9, 11, 11), 0);
    let d = dielectric();
    let (_, v_die, s_die, _) = capacitor(Some(&d));
    let e_die = eps_inv(&pwc_law_core::Reading { light: light(&d), ..Default::default() });
    let e_edge = (e_die + VACUUM) / 2;
    let predicted = f64::from(5 * VACUUM) / f64::from(2 * e_edge + 3 * e_die);
    println!("plate charge {q} (exact every tick); field inside a plate {inside}");
    println!("vacuum gap: V = {v_vac:.1} ± {s_vac:.1}, C = Q/V = {:.1}", q as f64 / v_vac);
    println!("dielectric gap: V = {v_die:.1} ± {s_die:.1}, C = {:.1}", q as f64 / v_die);
    // Ideal plates: C = A / Σ(ε⁻¹ of the gap's faces) = 256 / (5 · 1/2) for the vacuum gap. The rest
    // of the measured C is stray capacitance (fringes, outer faces), which the dielectric leaves alone.
    let ideal = 256.0 * 256.0 / f64::from(5 * VACUUM);
    let (c_vac, c_die) = (q as f64 / v_vac, q as f64 / v_die);
    let gain = (predicted - 1.0) * ideal;
    println!("ideal gap C {ideal:.1}, stray {:.1}; dielectric adds {:.1}, series-face prediction {gain:.1} (factor {predicted:.3}, eps {:.2})",
        c_vac - ideal, c_die - c_vac, f64::from(VACUUM) / f64::from(e_die));
    assert!(c_vac > ideal, "fringing can only add capacitance");
    assert!(((c_die - c_vac) - gain).abs() / gain < 0.05, "dielectric gain {}", c_die - c_vac);
    assert!(inside.abs() * 10 < v_vac as i64 / 5, "a conductor screens its inside");
}

/// A square loop of conductor (2×1 wire) in the plane z = 3 with a 2-cell gap in one side; the gap
/// holds `gap` (void when `None`). An optional closed ring sits at z = 5 above it.
fn lc(gap: Option<&Block>, ring: bool) -> Polyakov {
    let a = conductor();
    let mut m = Matter::new(Grid::new(24, 24, if ring { 9 } else { 7 }));
    let ia = m.intern(&a);
    let square = |m: &mut Matter, z: i32| {
        m.fill((4, 4, z), (19, 5, z), ia);
        m.fill((4, 18, z), (19, 19, z), ia);
        m.fill((4, 4, z), (5, 19, z), ia);
        m.fill((18, 4, z), (19, 19, z), ia);
    };
    square(&mut m, 3);
    if ring {
        square(&mut m, 5);
    }
    let gm = match gap {
        Some(d) => m.intern(d),
        None => VOID,
    };
    m.fill((18, 11, 3), (19, 12, 3), gm);
    Polyakov::new(m)
}

fn charge_gap(c: &mut Polyakov, s: i16) {
    let g = c.matter().grid;
    for x in 18..=19 {
        wilson(c, g.idx(x, 10, 3), 4, 3, s);
    }
}

/// Current along +y through the loop's far side (x = 4, 5) at y = 12, in the plane `z`.
fn side_current(c: &Polyakov, z: u32) -> i64 {
    let g = c.matter().grid;
    (4..=5).map(|x| i64::from(c.current(g.idx(x, 12, z), 1))).sum()
}

/// Swings of a series through ±`band` (hysteresis, so noise near zero does not count): (count, mean
/// ticks between them).
fn crossings(v: &[i64], band: i64) -> (usize, f64) {
    let mut at = Vec::new();
    let mut side = 0;
    for (t, &x) in v.iter().enumerate() {
        let now = if x > band { 1 } else if x < -band { -1 } else { side };
        if now != side && side != 0 {
            at.push(t);
        }
        side = now;
    }
    let gaps = if at.len() > 1 { (at[at.len() - 1] - at[0]) as f64 / (at.len() - 1) as f64 } else { 0.0 };
    (at.len(), gaps)
}

/// Charge on the loop's half below the gap (y ≤ 11) in the plane `z`.
fn lower_charge(c: &Polyakov, z: u32) -> i64 {
    sum_box(c, (0, 0, z), (23, 11, z), |i| i64::from(c.charge(i)))
}

#[test]
fn an_lc_loop_rings_and_a_dielectric_slows_it() {
    let d = dielectric();
    let mut periods = Vec::new();
    for (name, gap, around) in [("void", None, false), ("dielectric gap", Some(&d), false), ("in dielectric", Some(&d), true)] {
        let mut c = lc(gap, false);
        if around {
            let id = c.matter.intern(&d);
            let g = c.matter.grid;
            for i in 0..g.len() {
                if c.matter.get(i) == VOID {
                    c.matter.set(i, id);
                }
            }
            c = Polyakov::new(c.matter.clone());
        }
        charge_gap(&mut c, 3000);
        let mut q = Vec::new();
        for _ in 0..2400 {
            q.push(lower_charge(&c, 3));
            c.step();
        }
        let (n, half) = crossings(&q, 1500);
        let late = q[2000..].iter().map(|x| x.abs()).max().unwrap_or(0);
        if !around {
            println!("{name} loop, plane z = 3, tick 2400:\n{}", show::slice(&c, 3));
        }
        println!("{name}: lower-half charge every 20 ticks {:?}", q.iter().step_by(20).take(60).collect::<Vec<_>>());
        println!("  {n} swings, period {:.1} ticks, |Q| peak in ticks 2000..2400 {late}, Joule {}", 2.0 * half, c.joule());
        assert!(n >= 12 && late >= 6000, "{name}: the loop should keep ringing");
        periods.push(2.0 * half);
    }
    println!("periods: void gap {:.1}, dielectric gap {:.1}, loop in dielectric {:.1} (n = {:.2})", periods[0], periods[1], periods[2],
        (f64::from(VACUUM) / f64::from(eps_inv(&pwc_law_core::Reading { light: light(&d), ..Default::default() }))).sqrt());
    assert!(periods[1] > periods[0] && periods[2] > 1.5 * periods[0]);
}

#[test]
fn a_changing_current_induces_an_opposite_one_in_a_closed_ring() {
    let mut c = lc(None, true);
    charge_gap(&mut c, 3000);
    let ring = |c: &Polyakov| sum_box(c, (0, 0, 5), (23, 23, 5), |i| i64::from(c.charge(i)));
    let (mut i1, mut i2) = (Vec::new(), Vec::new());
    for t in 0..1200 {
        assert_eq!(ring(&c), 0, "the ring is an island: its charge stays 0 (tick {t})");
        i1.push(side_current(&c, 3));
        i2.push(side_current(&c, 5));
        c.step();
    }
    println!("ring plane z = 5 at tick 1200:\n{}", show::slice(&c, 5));
    let dot: i64 = i1.iter().zip(&i2).map(|(a, b)| a * b).sum();
    let n1: f64 = i1.iter().map(|a| (a * a) as f64).sum::<f64>().sqrt();
    let n2: f64 = i2.iter().map(|a| (a * a) as f64).sum::<f64>().sqrt();
    println!("primary far-side current every 10 ticks {:?}", i1.iter().step_by(10).take(50).collect::<Vec<_>>());
    println!("ring far-side current every 10 ticks    {:?}", i2.iter().step_by(10).take(50).collect::<Vec<_>>());
    let corr = dot as f64 / (n1 * n2);
    println!("correlation {corr:.3}, rms ratio ring/primary {:.3}", n2 / n1);
    assert!(corr < -0.2, "Lenz: the induced current opposes the inducing one");
    assert!(n2 > 0.3 * n1, "the ring carries a real current");
}

/// A hollow cube shell 10³ (wall 1) of `wall` around the centre of a 28³ box, and a charge pair
/// ±3000 outside it at x = 3 and x = 24, joined by a Wilson line routed around the shell. Over ticks
/// 3000..4000: the mean field energy inside the cavity, and the static (time-averaged) flux through
/// the cavity's central face.
fn shell(wall: Option<&Block>) -> (f64, f64, String) {
    let mut m = Matter::new(Grid::new(28, 28, 28));
    if let Some(w) = wall {
        let iw = m.intern(w);
        m.fill((9, 9, 9), (18, 18, 18), iw);
        m.fill((10, 10, 10), (17, 17, 17), VOID);
    }
    let mut c = Polyakov::new(m);
    let g = c.matter().grid;
    let up = wilson(&mut c, g.idx(3, 14, 14), 4, 8, 3000);
    let over = wilson(&mut c, up, 3, 21, 3000);
    wilson(&mut c, over, 1, 8, 3000);
    let (mut u, mut d) = (0.0, 0.0);
    for t in 0..4000 {
        c.step();
        if t >= 3000 {
            u += cavity_energy(&c) as f64;
            d += f64::from(c.flux(g.idx(14, 14, 14), 0));
        }
    }
    (u / 1000.0, (d / 1000.0).abs(), show::slice(&c, 14))
}

/// Field energy on the faces and edges strictly inside the cavity 10..=17.
fn cavity_energy(c: &Polyakov) -> i64 {
    let g = c.matter().grid;
    let mut u = 0i64;
    for z in 11..=17 {
        for y in 11..=17 {
            for x in 11..=17 {
                let i = g.idx(x, y, z);
                for k in 0..3 {
                    let dv = i64::from(c.flux(i, k));
                    let bv = i64::from(c.magnetic(i, k));
                    u += i64::from(c.eps_face(i, k)) * dv * dv + i64::from(VACUUM) * bv * bv;
                }
            }
        }
    }
    u / 256
}

#[test]
fn a_closed_conductor_shields_its_inside() {
    let (open, open_d, _) = shell(None);
    let (die, die_d, _) = shell(Some(&dielectric()));
    let (res, res_d, _) = shell(Some(&resistor()));
    let (con, con_d, view) = shell(Some(&conductor()));
    println!("conductor shell, z = 14, tick 4000 (the cavity stays blank):\n{view}");
    println!("cavity energy / static centre flux: no wall {open:.0} / {open_d:.2}, dielectric {die:.0} / {die_d:.2}, \
              resistor {res:.0} / {res_d:.2}, conductor {con:.0} / {con_d:.2}");
    println!("energy shielding: resistor {:.0}x, conductor {:.0}x", open / res.max(1.0), open / con.max(1.0));
    assert!(open_d > 2.0 && die_d > 1.0, "the static field reaches an unshielded cavity");
    assert!(res_d < open_d / 10.0 && con_d < open_d / 10.0, "a closed conductor screens the static field");
    assert!(con < open / 100.0 && res < open / 10.0, "a closed conductor keeps the field out");
}

/// Two incompatible conductors A (x 8..=13) and B (x 14..=19) touching across nine faces; a ground
/// plate G (x 1..=2) of A's material. Each input is a Wilson line of `q` from A to G. Returns the
/// charge that ended on B after 600 ticks.
fn junction(inputs: &[(u32, u32)], q: i16) -> (i64, i32, String) {
    let a = conductor();
    let r = rival(&a, 300);
    let mut m = Matter::new(Grid::new(24, 12, 12));
    let (ia, ir) = (m.intern(&a), m.intern(&r));
    m.fill((1, 1, 1), (2, 10, 10), ia);
    m.fill((8, 4, 4), (13, 6, 6), ia);
    m.fill((14, 4, 4), (19, 6, 6), ir);
    let mut c = Polyakov::new(m);
    let g = c.matter().grid;
    let t = c.face(g.idx(14, 5, 5), 0).t;
    for &(y, z) in inputs {
        wilson(&mut c, g.idx(8, y, z), 0, 6, q);
    }
    for _ in 0..600 {
        c.step();
    }
    (sum_box(&c, (14, 4, 4), (19, 6, 6), |i| i64::from(c.charge(i))), t, show::slice(&c, 5))
}

#[test]
fn an_incompatible_junction_is_a_threshold_gate() {
    let table = |q: i16| {
        let (a, t, _) = junction(&[(4, 5)], q);
        let (b, _, _) = junction(&[(6, 5)], q);
        let (both, _, view) = junction(&[(4, 5), (6, 5)], q);
        println!("q {q} per input (barrier T {t}): 00 -> 0, 10 -> {a}, 01 -> {b}, 11 -> {both}");
        if q == 2800 {
            println!("both inputs, z = 5 (G left, A | B right):\n{view}");
        }
        (a, b, both)
    };
    // AND: each input alone stays under the barrier; together they break through.
    let (a, b, both) = table(2800);
    assert!(a == 0 && b == 0 && both > 500);
    println!("AND gain: {both} out of 5600 in = {:.2}", both as f64 / 5600.0);
    // OR: either input alone breaks through.
    let (a, b, both) = table(4000);
    assert!(a > 200 && b > 200 && both > a + b);
    println!("OR gain: {both} out of 8000 in = {:.2}", both as f64 / 8000.0);
    // No gain: what crosses is always less than what came in, so gates cannot drive gates.
    for q in [1000, 2000, 3000, 6000, 12000] {
        let (_, _, both) = table(q);
        assert!(both < 2 * i64::from(q));
    }
}

/// `(Σ|m|, Σm, vertices where div B ≠ 0 mod 65536)`, as [`Polyakov::monopoles`].
type Monopoles = (i64, i64, usize);

/// Two opposite kicks of `a` on neighbouring faces of a vacuum box: their shared edge gets `−2a`.
/// Returns (monopoles right after, the shared edge's time-averaged B over ticks 1000..2000, max
/// monopole count seen).
fn kicked(a: i16) -> (Monopoles, f64, i64, Monopoles, String) {
    let mut c = Polyakov::new(Matter::new(Grid::new(14, 14, 14)));
    let g = c.matter().grid;
    let i = g.idx(6, 7, 7);
    c.kick(i, 2, a);
    c.kick(i + 1, 2, a.wrapping_neg());
    let first = c.monopoles();
    let (mut mean, mut most) = (0.0, 0);
    for t in 0..2000 {
        c.step();
        let (n, sum, bad) = c.monopoles();
        assert_eq!((sum, bad), (0, 0), "monopoles come in pairs and div B ≡ 0 mod 65536 (tick {t})");
        assert_eq!(c.gauss_defects().1, 0, "Gauss ≡ 0 mod 65536 (tick {t})");
        most = most.max(n);
        if t >= 1000 {
            mean += f64::from(c.magnetic(i + 1, 1));
        }
    }
    (first, mean / 1000.0, most, c.monopoles(), show::slice(&c, 7))
}

#[test]
fn a_field_driven_past_the_wrap_tears_into_a_monopole_pair_that_stays_a_magnet() {
    let (m0, b0, most0, _, _) = kicked(10000);
    let (m1, b1, most1, last, view) = kicked(20000);
    println!("z = 7 at tick 2000, the pair sits mid-box:\n{view}");
    println!("kicks 10000: monopoles {m0:?}, max {most0}, late mean B on the shared edge {b0:.1}");
    println!("kicks 20000 (edge −40000 wraps to +25536): monopoles {m1:?}, max {most1}, late mean B {b1:.1}");
    println!("a monopole is div B = ±{WRAP}: the Dirac string is the wrap itself, invisible to the law");
    assert_eq!(m0, (0, 0, 0));
    assert_eq!(m1, (2, 0, 0), "one wrapped edge = one monopole–antimonopole pair");
    assert_eq!(last, (2, 0, 0), "the pair is still there after 2000 ticks");
    assert!(b0.abs() < 50.0, "without monopoles a kick rings away to no static field");
    assert!(b1.abs() > 1000.0, "the pair holds a static magnetic field: a permanent magnet");
}

#[test]
fn coefficients_near_one_break_the_cfl_bound_and_blow_up() {
    let mut tops = Vec::new();
    for unit in [false, true] {
        let mut c = Polyakov::new(Matter::new(Grid::new(16, 16, 16)));
        if unit {
            c.mu = 255;
            for v in &mut c.eps {
                v.fill(255);
            }
        }
        let g = c.matter().grid;
        c.kick(g.idx(8, 8, 8), 2, 10000);
        let u0 = c.energy();
        let mut top = u0;
        for _ in 0..300 {
            c.step();
            top = top.max(c.energy());
        }
        println!("eps_inv·mu_inv = {}: energy start {u0}, max over 300 ticks {top} ({:.1}x), monopoles {:?}",
            if unit { "(255/256)^2" } else { "1/4" }, top as f64 / u0 as f64, c.monopoles());
        tops.push(top as f64 / u0 as f64);
        if unit {
            println!("z = 8 after 300 ticks:\n{}", show::slice(&c, 8));
        }
    }
    // The plain energy swings between D and B even in exact arithmetic; only the unstable law grows.
    assert!(tops[0] < 5.0 && tops[1] > 1000.0);
}

/// A vacuum box with one kick of `a`: the shadow energy at the start and its largest excursion
/// over `ticks`.
fn heating(a: i16, ticks: usize) -> (i64, i64) {
    let mut c = Polyakov::new(Matter::new(Grid::new(16, 16, 16)));
    let g = c.matter().grid;
    c.kick(g.idx(8, 8, 8), 2, a);
    c.step();
    let q0 = c.shadow_energy();
    let mut dev = 0i64;
    for t in 0..ticks {
        c.step();
        if t % 25 == 0 {
            dev = dev.max((c.shadow_energy() - q0).abs());
        }
    }
    if a == 100 && ticks == 8000 {
        println!("a kick of 100 after 8000 ticks, z = 8: rounding noise fills the box\n{}", show::slice(&c, 8));
    }
    (q0, dev)
}

#[test]
fn rounding_heats_the_vacuum_slowly_whatever_the_amplitude() {
    // The exact-arithmetic leapfrog conserves the shadow energy. Rounding breaks it by the same
    // absolute amount at every amplitude: a random walk of rounding errors, not an instability.
    let mut rates = Vec::new();
    for a in [100i16, 1000, 10000] {
        for ticks in [2000, 8000] {
            let (q0, dev) = heating(a, ticks);
            println!("kick {a}: shadow energy {q0}, largest drift over {ticks} ticks {dev} ({:.3}%), {:.0} per tick",
                100.0 * dev as f64 / q0 as f64, dev as f64 / ticks as f64);
            rates.push(dev as f64 / ticks as f64);
        }
    }
    let (lo, hi) = rates.iter().fold((f64::MAX, 0.0f64), |(l, h), &r| (l.min(r), h.max(r)));
    println!("drift {lo:.0}..{hi:.0} per tick on 4096 cells: {:.2} per cell per tick", hi / 4096.0);
    assert!(hi < 1.3 * lo, "the drift does not depend on amplitude");
    assert!(hi / 4096.0 < 1.0, "heating faster than measured");
}
