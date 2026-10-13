//! The experiments. Each builds real matter with `pwc_law_core::search`, runs the law, prints a
//! view under `--nocapture` and asserts the measured phenomenon. Failures of the law are kept as
//! tests that demonstrate them.

use crate::material::Block;
use crate::*;
use pwc_law_core::search::{Stream, bond, find, is_cohesive, mix};
use pwc_law_core::{Candidate, Grid, Matter, show};

const X_UP: usize = 3;
const X_DOWN: usize = 0;
const Y_DOWN: usize = 1;

fn energy(z: &Zeldovich) -> i64 {
    z.ledger().get("energy").expect("energy entry")
}

/// The wire: a cohesive 16-occurrence mixture that conducts to itself and whose every occurrence
/// sits behind a barrier of at least 150 quanta, so pulses of tens of quanta pass it untouched.
fn wire() -> Block {
    find(1, 3, 16, 100_000, |b| is_cohesive(b) && bond(b, b) > 0 && min_barrier(b) >= 150)
}

/// A second, different wire material (for the one-speed experiment).
fn other_wire(w: &Block) -> Block {
    find(2, 3, 16, 100_000, |b| {
        is_cohesive(b)
            && bond(b, b) > 0
            && min_barrier(b) >= 150
            && bond(b, w) > 0
            && b.configuration() != w.configuration()
    })
}

/// An 8-occurrence mixture of two or three elements, drawn from the stream.
fn mixture(s: &mut Stream) -> Block {
    let k = 2 + s.below(2) as usize;
    let parts: Vec<_> = (0..k).map(|i| (s.element(), 8 / k + usize::from(i < 8 % k))).collect();
    mix(&parts)
}

/// The first pair of cohesive mixtures from a seeded stream that satisfies `want`.
fn pair(seed: u64, tries: usize, want: impl Fn(&Block, &Block) -> bool) -> (Block, Block) {
    let mut s = Stream::new(seed);
    for _ in 0..tries {
        let (x, y) = (mixture(&mut s), mixture(&mut s));
        if is_cohesive(&x) && is_cohesive(&y) && want(&x, &y) {
            return (x, y);
        }
    }
    panic!("no pair in {tries} candidates of seed {seed}");
}

/// The smallest impact at which `x | y` reacts (its ignition threshold), up to `max`.
fn threshold(x: &Block, y: &Block, max: i64) -> Option<i64> {
    (1..=max).find(|&h| react(x, y, h).is_some())
}

/// A 1-D world: the blocks in order along x.
fn line(cells: &[&Block]) -> Zeldovich {
    let mut m = Matter::new(Grid::new(cells.len() as u32, 1, 1));
    for (i, b) in cells.iter().enumerate() {
        let id = m.intern(b);
        m.set(i, id);
    }
    Zeldovich::new(m)
}

/// `n` copies of a block.
fn run(b: &Block, n: usize) -> Vec<&Block> {
    vec![b; n]
}

fn total(z: &Zeldovich, cells: std::ops::Range<usize>) -> u64 {
    cells.map(|c| z.quanta(c)).sum()
}

/// One row of a 1-D world as digits: `.` empty, 1-9 quanta, `x` tens, `X` hundreds or more.
fn row(z: &Zeldovich) -> String {
    (0..z.matter().grid.nx as usize)
        .map(|i| match z.quanta(i) {
            0 => '.',
            q @ 1..=9 => (b'0' + q as u8) as char,
            10..=99 => 'x',
            _ => 'X',
        })
        .collect()
}

/// What one run of the fuel junction did.
struct Shot {
    out: u64,
    back: u64,
    reactions: u64,
    wire_kept: bool,
}

/// The fuel junction `[W×20] F O [W×60]` fed by pulses heading +x from cell 1 (`(tick, size)`):
/// quanta right of the junction, left of it, moves made, and whether every wire cell is intact.
fn shoot(w: &Block, f: &Block, o: &Block, pulses: &[(usize, u32)], ticks: usize) -> (Zeldovich, Shot) {
    let mut cells = run(w, 20);
    cells.push(f);
    cells.push(o);
    cells.extend(run(w, 60));
    let mut z = line(&cells);
    let wid = z.matter().get(0);
    let mut reactions = 0;
    for t in 0..ticks {
        for &(at, n) in pulses {
            if at == t {
                z.kick(1, X_UP, n);
            }
        }
        z.step();
        reactions += z.reactions();
    }
    let wire_kept = (0..20).chain(22..82).all(|c| z.matter().get(c) == wid);
    let shot = Shot { out: total(&z, 22..82), back: total(&z, 0..20), reactions, wire_kept };
    (z, shot)
}

/// The fuel pair of the devices: ignites at an impact `θ` between 10 and 40, releases at least
/// `2θ`, reflects any smaller pulse (the junction does not conduct), and fires exactly once for
/// any pulse from θ to 3θ without changing the wire. Found by running the device itself on each
/// candidate.
fn fuel(w: &Block) -> (Block, Block, i64) {
    let (f, o) = pair(99, 400_000, |f, o| {
        let Some(th) = threshold(f, o, 40) else { return false };
        let mv = react(f, o, th).expect("reacts at threshold");
        if th < 10 || mv.release < 2 * th || bond(f, o) > 0 || bond(w, f) <= 0 || bond(o, w) <= 0 {
            return false;
        }
        let (_, below) = shoot(w, f, o, &[(0, th as u32 - 1)], 45);
        below.out == 0
            && below.reactions == 0
            && [1, 2, 3].iter().all(|&k| {
                let (_, above) = shoot(w, f, o, &[(0, k * th as u32)], 45);
                above.reactions == 1 && above.wire_kept && above.out > 0
            })
    });
    let th = threshold(&f, &o, 40).expect("threshold");
    (f, o, th)
}

// ---------------------------------------------------------------------------------------------
// The ledger.

#[test]
fn ledger_closes_exactly_over_ten_thousand_ticks() {
    let w = wire();
    let (f, o, th) = fuel(&w);
    // A closed 8³ box: a 3-D checkerboard fuel bed (every face fuel | oxidiser) under a slab of
    // wire and a void sky, lit at one corner of the bed and heated on top of the wire.
    let g = Grid::new(8, 8, 8);
    let mut m = Matter::new(g);
    let (iw, i_f, io) = (m.intern(&w), m.intern(&f), m.intern(&o));
    for z in 0..8 {
        for y in 0..6 {
            for x in 0..8 {
                let fuel_bed = y < 3;
                m.put(
                    x,
                    y,
                    z,
                    if !fuel_bed {
                        iw
                    } else if (x + y + z) % 2 == 0 {
                        i_f
                    } else {
                        io
                    },
                );
            }
        }
    }
    let make = || {
        let mut u = Zeldovich::new(m.clone());
        u.poke(g.idx(0, 0, 0), 600);
        u.poke(g.idx(4, 5, 4), 900);
        u
    };
    let mut u = make();
    let start = u.ledger();
    let (e0, occ0) = (energy(&u), start.get("occurrences"));
    let mut reactions = 0u64;
    let mut quanta_range = (i64::MAX, i64::MIN);
    for _ in 0..10_000 {
        u.step();
        let l = u.ledger();
        assert_eq!(l.get("energy"), Some(e0), "energy drifted");
        assert_eq!(l.get("occurrences"), occ0, "elements were minted or lost");
        reactions += u.reactions();
        let q = l.get("quanta").unwrap();
        quanta_range = (quanta_range.0.min(q), quanta_range.1.max(q));
    }
    let end = u.ledger();
    println!("{}", show::slice(&u, 6));
    println!(
        "fuel threshold {th}; start {start:?}\nend   {end:?}\nreactions {reactions}, quanta ranged {quanta_range:?}, palette {}",
        u.matter().palette_len()
    );
    // The books moved: chemistry happened and converted energy both ways, yet E never changed.
    assert!(reactions > 1000);
    assert_ne!(end.get("binding"), start.get("binding"));
    assert!(quanta_range.1 > quanta_range.0);
    // Determinism: a second universe built the same way reaches the same bytes.
    let mut v = make();
    for _ in 0..10_000 {
        v.step();
    }
    assert_eq!(u.digest(), v.digest());
}

#[test]
fn cold_matter_is_frozen_and_costs_nothing() {
    let w = wire();
    let (f, o, _) = fuel(&w);
    let mut cells = run(&w, 5);
    for _ in 0..10 {
        cells.push(&f);
        cells.push(&o);
    }
    let mut z = line(&cells);
    let d0 = z.digest();
    for _ in 0..1000 {
        z.step();
        assert_eq!(z.active(), 0);
    }
    // Metastable fuel beside fuel, untouched for a thousand ticks: nothing happens without quanta.
    assert_eq!(z.digest(), d0);
}

// ---------------------------------------------------------------------------------------------
// Sound and signals.

#[test]
fn pulses_travel_one_cell_per_tick_turn_corners_and_fan_out() {
    let w = wire();
    // An L: along x from (0,0) to (9,0), then along y to (9,9).
    let g = Grid::new(10, 10, 1);
    let mut m = Matter::new(g);
    let iw = m.intern(&w);
    m.fill((0, 0, 0), (9, 0, 0), iw);
    m.fill((9, 0, 0), (9, 9, 0), iw);
    let mut z = Zeldovich::new(m);
    z.kick(g.idx(0, 0, 0), X_UP, 12);
    let end = g.idx(9, 9, 0);
    let mut arrived = None;
    for t in 1..=40 {
        z.step();
        if arrived.is_none() && z.quanta(end) > 0 {
            arrived = Some((t, z.quanta(end)));
        }
    }
    println!("{}", show::slice(&z, 0));
    // Eighteen cells of path, eighteen ticks: the corner costs nothing and keeps every quantum.
    assert_eq!(arrived, Some((18, 12)), "the whole pulse arrives after exactly the path length");

    // A T: the input along x meets a crossbar along y and splits evenly into both arms.
    let g = Grid::new(6, 11, 1);
    let mut m = Matter::new(g);
    let iw = m.intern(&w);
    m.fill((0, 5, 0), (5, 5, 0), iw);
    m.fill((5, 0, 0), (5, 10, 0), iw);
    let mut z = Zeldovich::new(m);
    z.kick(g.idx(0, 5, 0), X_UP, 40);
    let (top, bottom) = (g.idx(5, 10, 0), g.idx(5, 0, 0));
    let (mut t_top, mut t_bottom) = (None, None);
    for t in 1..=20 {
        z.step();
        if t_top.is_none() && z.quanta(top) > 0 {
            t_top = Some((t, z.quanta(top)));
        }
        if t_bottom.is_none() && z.quanta(bottom) > 0 {
            t_bottom = Some((t, z.quanta(bottom)));
        }
    }
    println!("{}", show::slice(&z, 0));
    println!("T junction: top {t_top:?}, bottom {t_bottom:?}");
    assert_eq!(t_top, Some((10, 20)));
    assert_eq!(t_bottom, Some((10, 20)));
}

#[test]
fn a_bar_rings_forever_with_period_twice_its_length() {
    let w = wire();
    for len in [5usize, 16, 33] {
        let mut z = line(&run(&w, len));
        z.kick(0, X_UP, 7);
        let start = z.digest();
        let mut returns = Vec::new();
        for t in 1..=2000 * len {
            z.step();
            if z.digest() == start {
                returns.push(t);
            }
        }
        // The state recurs exactly every 2L ticks, a thousand times over: a clock.
        assert_eq!(returns.len(), 1000);
        assert!(returns.iter().enumerate().all(|(k, &t)| t == 2 * len * (k + 1)));
        println!("bar of {len}: period {} ticks, {} exact returns", returns[0], returns.len());
    }
}

// ---------------------------------------------------------------------------------------------
// Heat.

/// Two 4³ blocks of wire along x, touching or separated by a one-cell layer of `gap`.
fn two_blocks(w: &Block, gap: Option<&Block>) -> (Zeldovich, Grid) {
    let nx = if gap.is_some() { 9 } else { 8 };
    let g = Grid::new(nx, 4, 4);
    let mut m = Matter::new(g);
    let iw = m.intern(w);
    m.fill((0, 0, 0), (3, 3, 3), iw);
    m.fill((nx as i32 - 4, 0, 0), (nx as i32 - 1, 3, 3), iw);
    if let Some(i) = gap {
        let ii = m.intern(i);
        m.fill((4, 0, 0), (4, 3, 3), ii);
    }
    (Zeldovich::new(m), g)
}

fn side(z: &Zeldovich, g: Grid, right: bool) -> u64 {
    let x0 = if right { g.nx - 4 } else { 0 };
    let mut sum = 0;
    for z_ in 0..4 {
        for y in 0..4 {
            for x in x0..x0 + 4 {
                sum += z.quanta(g.idx(x, y, z_));
            }
        }
    }
    sum
}

#[test]
fn heat_conducts_across_bonds_and_stops_at_an_insulator() {
    let w = wire();
    // An insulator: cohesive, high barriers, and repelled by the wire (bond ≤ 0).
    let ins = find(3, 3, 16, 100_000, |b| is_cohesive(b) && min_barrier(b) >= 150 && bond(b, &w) <= 0);
    for (name, gap) in [("touching", None), ("insulated", Some(&ins))] {
        let (mut z, g) = two_blocks(&w, gap);
        for k in 0..5 {
            z.poke(g.idx(1 + k % 2, 1 + k / 2 % 2, 1 + k / 4), 600);
        }
        let e0 = energy(&z);
        let mut moves = 0;
        for _ in 0..3000 {
            z.step();
            moves += z.reactions();
        }
        assert_eq!(energy(&z), e0);
        assert_eq!(moves, 0);
        let (l, r) = (side(&z, g, false), side(&z, g, true));
        println!("{name}: left {l}, right {r}");
        println!("{}", show::slice(&z, 1));
        if gap.is_some() {
            assert_eq!(r, 0, "not one quantum crosses a non-bonded face");
            assert_eq!(l, 3000);
        } else {
            // Contact shares the heat: the far block ends with a third or more of it.
            assert!(r * 3 >= 3000, "right block got only {r}");
        }
    }
}

#[test]
fn heat_spreads_to_equal_energy_per_bond() {
    let w = wire();
    let g = Grid::new(8, 8, 8);
    let mut m = Matter::new(g);
    let iw = m.intern(&w);
    m.fill((0, 0, 0), (7, 7, 7), iw);
    let mut z = Zeldovich::new(m);
    // Eight central cells at 600 quanta each: 100 per face, below the wire's barriers.
    for c in [(3, 3, 3), (4, 3, 3), (3, 4, 3), (4, 4, 3), (3, 3, 4), (4, 3, 4), (3, 4, 4), (4, 4, 4)] {
        z.poke(g.idx(c.0, c.1, c.2), 600);
    }
    let e0 = energy(&z);
    let mut moves = 0;
    let mut spread = Vec::new();
    let mut mean = vec![0u64; g.len()];
    for t in 1..=4000 {
        z.step();
        moves += z.reactions();
        if [1, 2, 4, 8, 16].contains(&t) {
            spread.push((t, (0..g.len()).filter(|&c| z.quanta(c) > 0).count()));
        }
        if t > 2000 {
            for (c, s) in mean.iter_mut().enumerate() {
                *s += z.quanta(c);
            }
        }
    }
    assert_eq!(energy(&z), e0);
    assert_eq!(moves, 0);
    // Dealing is doubly stochastic, so the stationary flow is equal on every conducting face:
    // a cell's energy is proportional to its bonds (interior 6, face 5, edge 4, corner 3).
    let bonds: u64 = (0..g.len()).map(|c| u64::from(z.open(c).count_ones())).sum();
    let per_bond = 4800.0 / bonds as f64;
    let mut class = [(0u64, 0u64); 4];
    for (c, &s) in mean.iter().enumerate() {
        let k = 6 - z.open(c).count_ones() as usize;
        class[k].0 += s;
        class[k].1 += 2000;
    }
    println!("cells holding quanta at tick: {spread:?}");
    println!("{}", show::slice(&z, 3));
    for (k, name) in ["interior", "face", "edge", "corner"].iter().enumerate() {
        let got = class[k].0 as f64 / class[k].1 as f64;
        let want = per_bond * (6 - k) as f64;
        println!("{name}: {got:.3} quanta per cell over ticks 2001..4000, predicted {want:.3}");
        assert!((got / want - 1.0).abs() < 0.01, "{name}: {got:.3} vs {want:.3}");
    }
    assert_eq!(spread.last().unwrap().1, g.len());
}

// ---------------------------------------------------------------------------------------------
// Chemistry.

#[test]
fn a_fuel_ignites_only_above_a_temperature_after_a_delay() {
    let w = wire();
    // A conducting fuel pair (bond > 0, so heat flows through it) buried in a 6³ block of wire.
    let (f, o) = pair(11, 400_000, |f, o| {
        bond(f, o) > 0
            && bond(&w, f) > 0
            && bond(o, &w) > 0
            && threshold(f, o, 60).is_some_and(|th| th >= 6 && react(f, o, th).unwrap().release >= th)
    });
    let th = threshold(&f, &o, 60).unwrap();
    // Heat the whole block evenly, `per` quanta per cell, and wait up to 500 ticks.
    let ignite = |per: i32| -> Option<(usize, bool)> {
        let g = Grid::new(6, 6, 6);
        let mut m = Matter::new(g);
        let (iw, i_f, io) = (m.intern(&w), m.intern(&f), m.intern(&o));
        m.fill((0, 0, 0), (5, 5, 5), iw);
        m.put(2, 3, 3, i_f);
        m.put(3, 3, 3, io);
        let mut z = Zeldovich::new(m);
        for c in 0..g.len() {
            z.poke(c, per);
        }
        for t in 1..=500 {
            z.step();
            if z.reactions() > 0 {
                let at_junction = z.matter().get(g.idx(2, 3, 3)) != i_f || z.matter().get(g.idx(3, 3, 3)) != io;
                return Some((t, at_junction));
            }
        }
        None
    };
    let sweep: Vec<(i32, Option<(usize, bool)>)> = (6..=96).step_by(6).map(|per| (per, ignite(per))).collect();
    println!("fuel threshold {th}; quanta per cell -> first move (tick, at the junction):");
    for (per, r) in &sweep {
        println!("  {per:3} ({:.1} per face): {r:?}", f64::from(*per) / 6.0);
    }
    // Cold: never. Hot: at the junction first. The delay shrinks as the heat grows.
    let first = sweep.iter().position(|(_, r)| r.is_some()).expect("ignites when hot");
    assert!(first > 0, "ignites even at the lowest heat");
    assert!(sweep[first..].iter().all(|(_, r)| r.is_some_and(|(_, j)| j)));
    let delays: Vec<usize> = sweep[first..].iter().map(|(_, r)| r.unwrap().0).collect();
    assert!(delays.windows(2).all(|d| d[1] <= d[0]), "delay not shrinking: {delays:?}");
    assert!(delays[0] > delays[delays.len() - 1]);
}

#[test]
fn a_reaction_happens_only_above_an_impact_threshold() {
    let w = wire();
    let (f, o, th) = fuel(&w);
    let mv = react(&f, &o, th).unwrap();
    println!("fuel: threshold {th}, barrier {}, release {}, gain {}", mv.barrier, mv.release, mv.gain);
    for h in 0..th {
        assert_eq!(react(&f, &o, h), None, "reacted below threshold at {h}");
    }
    assert!(mv.barrier <= th && mv.release > 0);
    // The barrier is real: the release would pay for it, but cold fuel waits forever.
    assert!(mv.release >= 2 * mv.barrier);
}

#[test]
fn a_flame_front_is_a_pressure_wave() {
    let w = wire();
    let (f, o, th) = fuel(&w);
    // A rod of alternating fuel and oxidiser, ignited at one end by a pulse at the threshold.
    let n = 64;
    let mut cells = Vec::new();
    for i in 0..n {
        cells.push(if i % 2 == 0 { &f } else { &o });
    }
    let mut z = line(&cells);
    let base: Vec<_> = (0..n).map(|c| z.matter().get(c)).collect();
    z.kick(0, X_UP, th as u32);
    let e0 = energy(&z);
    let mut burnt_at = vec![None; n];
    let mut front_share = Vec::new();
    for t in 1..=48 {
        z.step();
        for (c, b) in burnt_at.iter_mut().enumerate() {
            if b.is_none() && z.matter().get(c) != base[c] {
                *b = Some(t);
            }
        }
        if t <= 20 || t % 4 == 0 {
            println!("{t:3} {}  reactions {}", row(&z), z.reactions());
        }
        // The leading cell holding quanta, and the pulse in it heading on (+x).
        let lead = (0..n).rev().find(|&c| z.quanta(c) > 0).unwrap();
        front_share.push((t, lead, z.channels(lead)[X_UP], total(&z, 0..n)));
    }
    assert_eq!(energy(&z), e0);
    // Speed: the cell first burnt at each tick; fit cells over ticks between 10 and 40.
    let first = burnt_at.iter().position(|b| b.is_some_and(|t| t >= 10)).unwrap();
    let last = burnt_at.iter().rposition(|b| b.is_some_and(|t| t <= 40)).unwrap();
    let speed = (last - first) as f64 / (burnt_at[last].unwrap() - burnt_at[first].unwrap()) as f64;
    let pulses: Vec<u32> = front_share[9..].iter().map(|f| f.2).collect();
    let (lo, hi) = (*pulses.iter().min().unwrap(), *pulses.iter().max().unwrap());
    let (t, lead, _, all) = front_share[47];
    let release = react(&f, &o, th).unwrap().release;
    println!(
        "front speed {speed:.3} cells/tick; leading pulse over ticks 10..=48: {lo}..={hi} quanta (ignition {th}, first release {release})"
    );
    println!("at tick {t}: front at cell {lead}, {all} quanta in the rod; palette {}", z.matter().palette_len());
    assert!((speed - 1.0).abs() < 0.05, "front speed {speed}");
    // A detonation: a steady shock moving at the speed of sound, carrying a pulse well above the
    // ignition threshold, sustained by the reaction it triggers.
    assert!(i64::from(lo) >= 2 * th, "front pulse fell to {lo}");
    assert!(hi <= 2 * lo, "front pulse unsteady: {lo}..{hi}");
    // Restoration: whatever lit it, the front settles to the same pulse (a logic level).
    let settled = |ignition: u32| -> u32 {
        let mut z = line(&cells);
        z.kick(0, X_UP, ignition);
        for _ in 0..40 {
            z.step();
        }
        let lead = (0..n).rev().find(|&c| z.quanta(c) > 0).unwrap();
        z.channels(lead)[X_UP]
    };
    let levels: Vec<(u32, u32)> = [th as u32, 2 * th as u32, 3 * th as u32].iter().map(|&i| (i, settled(i))).collect();
    println!("ignition -> leading pulse at tick 40: {levels:?}");
    assert!(levels.iter().all(|&(_, l)| l == levels[0].1));
}

#[test]
fn an_amplifier_one_small_pulse_releases_a_large_one() {
    let w = wire();
    let (f, o, th) = fuel(&w);
    let input = th as u32;
    let (z, shot) = shoot(&w, &f, &o, &[(0, input)], 45);
    println!("{}", row(&z));
    let gain = shot.out as f64 / f64::from(input);
    println!("in {input}, out {} (gain {gain:.2}), back {}, moves {}", shot.out, shot.back, shot.reactions);
    // One junction: out = (in + release) / 2, so release ≥ 2θ gives a gain of at least 1.5.
    assert!(2 * shot.out >= 3 * u64::from(input), "gain below 1.5");
    assert!(shot.wire_kept);
    // The fuel is spent. Long wires keep echoes away: a second identical pulse 100 ticks later
    // meets the fired junction.
    let mut cells = run(&w, 200);
    cells.push(&f);
    cells.push(&o);
    cells.extend(run(&w, 200));
    let mut z = line(&cells);
    let wid = z.matter().get(0);
    let mut outs = Vec::new();
    for _ in 0..2 {
        let before = total(&z, 202..402);
        z.kick(190, X_UP, input);
        let mut moves = 0;
        for _ in 0..100 {
            z.step();
            moves += z.reactions();
        }
        outs.push((moves, total(&z, 202..402) - before));
    }
    let kept = (0..200).chain(202..402).all(|c| z.matter().get(c) == wid);
    println!(
        "first pulse: {} move(s), out {}; second pulse: {} move(s), out {}; wire intact {kept}",
        outs[0].0, outs[0].1, outs[1].0, outs[1].1
    );
    assert_eq!(outs[0].0, 1);
    assert!(outs[1].1 <= u64::from(input), "the spent junction amplified again");
}

#[test]
fn a_threshold_gate_and_and_or_from_one_junction() {
    let w = wire();
    let (f, o, th) = fuel(&w);
    // Sub-threshold pulses reflect; at threshold the junction fires and passes an amplified pulse.
    for (input, fires) in [(1, false), (th - 1, false), (th, true), (2 * th, true), (3 * th, true)] {
        let (_, s) = shoot(&w, &f, &o, &[(0, input as u32)], 45);
        println!("threshold gate: in {input} -> out {} (moves {}, wire intact {})", s.out, s.reactions, s.wire_kept);
        assert_eq!(s.out > 0, fires);
        assert_eq!(s.reactions, u64::from(fires));
        assert!(s.wire_kept);
    }
    // Two inputs merge at a T junction J: A runs along x into J, B comes down along y. J deals
    // each over its other two faces, so half of each goes on toward the fuel (and half goes up
    // the other input). Downstream, the fuel junction fires iff the merged impact reaches θ.
    //   B wire: (8, 1..=8) heading −y; A wire: (0..8, 0); J = (8, 0); then wire to F, O, out.
    let gate = |a: u32, b: u32| -> u64 {
        let g = Grid::new(40, 9, 1);
        let mut m = Matter::new(g);
        let (iw, i_f, io) = (m.intern(&w), m.intern(&f), m.intern(&o));
        m.fill((0, 0, 0), (39, 0, 0), iw);
        m.fill((8, 1, 0), (8, 8, 0), iw);
        m.put(14, 0, 0, i_f);
        m.put(15, 0, 0, io);
        let mut z = Zeldovich::new(m);
        // A and B reach J on the same tick: both start eight cells away.
        z.kick(g.idx(8, 8, 0), Y_DOWN, b);
        z.kick(g.idx(0, 0, 0), X_UP, a);
        for _ in 0..30 {
            z.step();
        }
        (16..40).map(|x| z.quanta(g.idx(x, 0, 0))).sum()
    };
    let and = th as u32 + 1;
    let or = 2 * th as u32 + 1;
    assert!(and.div_ceil(2) < th as u32 && 2 * (and / 2) >= th as u32 && or / 2 >= th as u32);
    for (level, name) in [(and, "AND"), (or, "OR")] {
        let table: Vec<u64> =
            [(0, 0), (level, 0), (0, level), (level, level)].iter().map(|&(a, b)| gate(a, b)).collect();
        println!("{name} at pulse {level}: 00->{} 10->{} 01->{} 11->{}", table[0], table[1], table[2], table[3]);
        let bits: Vec<bool> = table.iter().map(|&v| v > 0).collect();
        match name {
            "AND" => assert_eq!(bits, [false, false, false, true]),
            _ => assert_eq!(bits, [false, true, true, true]),
        }
    }
}

/// What one charge-hold-discharge cycle of a junction `X Y` between long wires did.
struct Cycle {
    charge: u32,
    stored: i64,
    trigger: u32,
    released: i64,
    out: u64,
}

/// Charge the junction with the smallest pulse whose first move stores energy, hold it for
/// `hold` ticks, then fire it with the smallest impact that releases energy from the charged
/// pair. `None` unless every stage behaves and the wires stay intact.
fn cycle(w: &Block, x: &Block, y: &Block, hold: usize) -> Option<Cycle> {
    let charge = (1..=120).find(|&h| react(x, y, h).is_some_and(|m| m.release < 0))? as u32;
    let len = 400;
    let mut cells = run(w, len);
    cells.push(x);
    cells.push(y);
    cells.extend(run(w, len));
    let mut z = line(&cells);
    let wid = z.matter().get(0);
    let b0 = z.ledger().get("binding").unwrap();
    z.kick(len - 2, X_UP, charge);
    for _ in 0..100 {
        z.step();
    }
    let stored = z.ledger().get("binding").unwrap() - b0;
    let near = (len - 120..len + 122).any(|c| z.quanta(c) > 0);
    if stored >= 0 || near {
        return None;
    }
    let (bx, by) = (z.matter().block(z.matter().get(len)).clone(), z.matter().block(z.matter().get(len + 1)).clone());
    let trigger = (1..=120).find(|&h| react(&bx, &by, h).is_some_and(|m| m.release > 0))? as u32;
    for _ in 0..hold {
        z.step();
        if z.reactions() > 0 {
            return None;
        }
    }
    z.kick(len - 2, X_UP, trigger);
    for _ in 0..60 {
        z.step();
    }
    let released = z.ledger().get("binding").unwrap() - (b0 + stored);
    let kept = (0..len).chain(len + 2..2 * len + 2).all(|c| z.matter().get(c) == wid);
    let out = total(&z, len + 2..len + 70) + total(&z, len - 70..len);
    kept.then_some(Cycle { charge, stored, trigger, released, out })
}

#[test]
fn a_battery_stores_a_pulse_as_binding_and_returns_it_on_a_trigger() {
    let w = wire();
    // A junction whose first move at some impact stores energy (release < 0), whose charged state
    // then needs a trigger of at least five quanta (it survives smaller knocks) and gives the
    // stored energy back in full. Found by running the cycle on each candidate.
    let (x, y) = pair(7, 200_000, |x, y| {
        bond(&w, x) > 0
            && bond(y, &w) > 0
            && cycle(&w, x, y, 50).is_some_and(|c| c.trigger >= 5 && c.trigger < c.charge && c.released == -c.stored)
    });
    let c = cycle(&w, &x, &y, 5000).expect("the cycle holds for 5000 ticks too");
    println!(
        "charge pulse {}: binding {} (energy stored), held 5000 ticks; trigger {}: released {}, pulses out {} ({:.2}x the trigger)",
        c.charge,
        c.stored,
        c.trigger,
        c.released,
        c.out,
        c.out as f64 / f64::from(c.trigger)
    );
    // Vibration became binding, waited, and came back as vibration larger than the trigger.
    assert_eq!(c.released, -c.stored);
    assert_eq!(c.out, u64::from(c.trigger) + c.released as u64);
}

// ---------------------------------------------------------------------------------------------
// What the law cannot do.

#[test]
fn failure_pulses_never_interfere() {
    let w = wire();
    // Two equal pulses heading at each other pass through one another: quanta carry no phase, so
    // there is no cancellation, no standing-wave node and no diffraction.
    let mut z = line(&run(&w, 20));
    z.kick(0, X_UP, 9);
    z.kick(19, X_DOWN, 9);
    for t in 0..19 {
        if t % 2 == 0 {
            println!("{}", row(&z));
        }
        z.step();
    }
    assert_eq!(z.channels(0)[X_DOWN], 9);
    assert_eq!(z.channels(19)[X_UP], 9);
}

#[test]
fn failure_one_speed_for_all_matter() {
    let w = wire();
    let v = other_wire(&w);
    println!(
        "wire: binding {} barrier {}; other wire: binding {} barrier {}",
        binding(w.internal_fit()),
        min_barrier(&w),
        binding(v.internal_fit()),
        min_barrier(&v)
    );
    // Ten cells of each: the pulse takes ten ticks through either, whatever their stiffness.
    let mut cells = run(&w, 10);
    cells.extend(run(&v, 10));
    let mut z = line(&cells);
    z.kick(0, X_UP, 5);
    for _ in 0..10 {
        z.step();
    }
    assert_eq!(z.quanta(10), 5);
    for _ in 0..9 {
        z.step();
    }
    assert_eq!(z.quanta(19), 5);
}

#[test]
fn failure_dealing_is_not_reversible() {
    let w = wire();
    // A T junction J = (1,1) with arms −x, −y and +y. One quantum entering from −x and one
    // entering from +y are both dealt to −y by J's rotor: two pasts, one present.
    let g = Grid::new(3, 3, 1);
    let mut m = Matter::new(g);
    let iw = m.intern(&w);
    m.fill((0, 1, 0), (1, 1, 0), iw);
    m.fill((1, 0, 0), (1, 2, 0), iw);
    let mut a = Zeldovich::new(m.clone());
    a.kick(g.idx(0, 1, 0), X_UP, 1);
    let mut b = Zeldovich::new(m);
    b.kick(g.idx(1, 2, 0), Y_DOWN, 1);
    assert_ne!(a.digest(), b.digest());
    a.step();
    b.step();
    assert_eq!(a.channels(g.idx(1, 1, 0))[Y_DOWN], 1);
    assert_eq!(a.digest(), b.digest());
    assert!(!a.unstep());
}

#[test]
fn failure_a_big_fire_burns_the_wire_too() {
    let w = wire();
    let (f, o, _) = fuel(&w);
    // Pure wire shrugs off any heat: its only moves between equal cells have zero gain.
    let g = Grid::new(6, 6, 6);
    let mut m = Matter::new(g);
    let iw = m.intern(&w);
    m.fill((0, 0, 0), (5, 5, 5), iw);
    let mut z = Zeldovich::new(m);
    z.poke(g.idx(2, 2, 2), 60_000);
    let mut moves = 0;
    for _ in 0..2000 {
        z.step();
        moves += z.reactions();
    }
    println!("pure wire poked with 60000 quanta (10000 per face): {moves} moves");
    assert_eq!(moves, 0);
    // Fuel beds of 1 to 5 layers under the same wire in an 8³ box, lit at one corner. Fuel
    // atoms pushed into the hot wire seed a runaway in which wire cells empty into their
    // neighbours (binding grows like n², so clumping releases energy without bound).
    let mut runs = Vec::new();
    for layers in 1..=5 {
        let g = Grid::new(8, 8, 8);
        let mut m = Matter::new(g);
        let (iw, i_f, io) = (m.intern(&w), m.intern(&f), m.intern(&o));
        for z in 0..8 {
            for y in 0..8 {
                for x in 0..8 {
                    m.put(
                        x,
                        y,
                        z,
                        if y >= layers {
                            iw
                        } else if (x + y + z) % 2 == 0 {
                            i_f
                        } else {
                            io
                        },
                    );
                }
            }
        }
        let mut z = Zeldovich::new(m);
        z.poke(g.idx(0, 0, 0), 600);
        let e0 = energy(&z);
        let mut moves = 0;
        for _ in 0..1000 {
            z.step();
            moves += z.reactions();
        }
        assert_eq!(energy(&z), e0);
        let wire_cells = (0..g.len()).filter(|&c| g.pos(c).1 >= layers as u32).count();
        let changed = (0..g.len()).filter(|&c| g.pos(c).1 >= layers as u32 && z.matter().get(c) != iw).count();
        let q = z.ledger().get("quanta").unwrap();
        println!(
            "{layers} fuel layers: {moves} moves, quanta 600 -> {q}, palette {}, wire cells changed {changed}/{wire_cells}",
            z.matter().palette_len()
        );
        runs.push((q, changed, wire_cells));
    }
    // One layer burns out and stays small; five layers set the wire itself burning.
    assert!(runs[0].0 < 6000 && runs[0].1 * 10 < runs[0].2);
    assert!(runs[4].0 > 100_000 && runs[4].1 * 2 > runs[4].2);
}

#[test]
fn failure_turning_at_walls_would_condense_heat_on_edges() {
    // The rejected transport rule, run on its own: quanta go straight on through open faces and
    // only a quantum facing a wall turns, dealt over the open side faces (never back). It routes
    // pulses just as well, but its average is not doubly stochastic: edges and corners of a
    // convex body absorb the flow, so heat would gather there by itself (a second-law breach).
    let g = Grid::new(8, 8, 8);
    let open = |c: usize| -> u8 { (0..6).filter(|&d| g.step(c, d).is_some()).fold(0, |m, d| m | 1 << d) };
    let mut q = vec![[0u64; 6]; g.len()];
    let mut rotor = vec![0u8; g.len()];
    for c in [(3, 3, 3), (4, 3, 3), (3, 4, 3), (4, 4, 3), (3, 3, 4), (4, 3, 4), (3, 4, 4), (4, 4, 4)] {
        q[g.idx(c.0, c.1, c.2)] = [100; 6];
    }
    let mut mean = vec![0u64; g.len()];
    for t in 1..=4000 {
        let mut next = vec![[0u64; 6]; g.len()];
        for c in 0..g.len() {
            let o = open(c);
            for d in 0..6 {
                let n = q[c][d];
                if n == 0 {
                    continue;
                }
                if o & (1 << d) != 0 {
                    next[g.step(c, d).unwrap()][d] += n;
                    continue;
                }
                let sides = o & !(1 << d | 1 << pwc_law_core::grid::opposite(d));
                let k = u64::from(sides.count_ones());
                let (each, mut rest) = (n / k, n % k);
                let mut r = rotor[c] as usize;
                for (s, v) in next[c].iter_mut().enumerate() {
                    if sides & (1 << s) != 0 {
                        *v += each;
                    }
                }
                while rest > 0 {
                    r = (r + 1) % 6;
                    if sides & (1 << r) != 0 {
                        next[c][r] += 1;
                        rest -= 1;
                    }
                }
                rotor[c] = r as u8;
            }
        }
        q = next;
        if t > 2000 {
            for c in 0..g.len() {
                mean[c] += q[c].iter().sum::<u64>();
            }
        }
    }
    assert_eq!(q.iter().flatten().sum::<u64>(), 4800);
    let mut class = [(0u64, 0u64); 4];
    for (c, &s) in mean.iter().enumerate() {
        let k = 6 - open(c).count_ones() as usize;
        class[k].0 += s;
        class[k].1 += 2000;
    }
    let per: Vec<f64> = class.iter().map(|&(s, n)| s as f64 / n as f64).collect();
    println!(
        "turning rule, quanta per cell over ticks 2001..4000: interior {:.2}, face {:.2}, edge {:.2}, corner {:.2} (uniform 9.38)",
        per[0], per[1], per[2], per[3]
    );
    assert_eq!(class[0].0 + class[1].0, 0, "heat left the edges");
}
