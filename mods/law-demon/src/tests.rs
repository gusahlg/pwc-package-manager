//! The experiments. Every material comes from a deterministic search over the element stream, with
//! the reason it was chosen written as its predicate; nothing names an element.

use pwc_law_core::search::{bond, find, find_with, is_cohesive};
use pwc_law_core::{show, Candidate, Grid, Mat, Matter};

use super::{contact_moves, Demon};
use crate::material::{cohesion, fit_raw, Block};

/// Least heat any single cell needs before the ordered contact `(a, b)` can move at all.
fn ignition(a: &Block, b: &Block) -> u32 {
    contact_moves(a, b).iter().map(|m| m.need[0].max(m.need[1])).min().unwrap_or(u32::MAX)
}

/// The best (first-ranked) move of `a | b`: its binding changes and needs.
fn best(a: &Block, b: &Block) -> ([i32; 2], [u32; 2]) {
    let m = contact_moves(a, b)[0];
    (m.delta, m.need)
}

/// Fuel and oxidizer: a cohesive mixture F and a cohesive X that conducts well within itself and
/// across `F | X`, whose best move releases more heat than it needs but at most twice as much
/// (a fuel that burns without turning the rod into a plasma), and an inert, conductive sink S.
fn fuel() -> (Block, Block, Block) {
    let f = find(2, 3, 8, 100_000, |b| is_cohesive(b) && cohesion(b) > 40 && bond(b, b) > 20);
    let x = find_seq(100, |x| {
        if !(is_cohesive(x) && bond(x, x) > 300 && bond(&f, x) > 100) {
            return false;
        }
        let (d, n) = best(&f, x);
        let (rel, need) = (d[0].max(d[1]), n[0].max(n[1]) as i32);
        rel > need && rel <= 2 * need
    });
    let s = find(7002, 3, 19, 1_000_000, |s| {
        is_cohesive(s) && bond(s, s) > 150 && bond(s, &x) > 20 && ignition(s, s) > 3000
    });
    (f, x, s)
}

/// The first configuration `find(seed, 3, 8, ..)` for seeds from `from` on that satisfies `want`.
fn find_seq(from: u64, want: impl Fn(&Block) -> bool) -> Block {
    for seed in from..from + 1000 {
        let b = find(seed, 3, 8, 100_000, |b| is_cohesive(b) && bond(b, b) > 20);
        if want(&b) {
            return b;
        }
    }
    panic!("no seed in {from}..+1000 satisfies the search");
}

/// A pure block whose element repels every element of `others`: with a self-bond of 0 it never
/// conducts, and any move into it would have to be paid by its own heat, which it never gets.
fn firebreak(others: &[&Block]) -> Block {
    find_with(31, 4, 100_000, |b| {
        let i = b.elements()[0];
        others.iter().all(|o| o.elements().iter().all(|&e| fit_raw(i, e) < 0))
    })
}

fn assert_ledger(d: &Demon, e0: i64, occ0: i64) {
    assert_eq!(d.energy().2, e0, "energy must be conserved exactly");
    assert_eq!(d.matter().occurrences(), occ0, "elements are conserved");
}

/// A rod of fuel over oxidizer on a sink slab, `n` columns: rows 0..=5 sink, 6 oxidizer, 7 fuel.
fn rod(n: u32, f: &Block, x: &Block, s: &Block) -> (Demon, Mat, Mat) {
    let mut m = Matter::new(Grid::new(n, 8, 1));
    let (mf, mx, ms) = (m.intern(f), m.intern(x), m.intern(s));
    m.fill((0, 0, 0), (n as i32 - 1, 5, 0), ms);
    m.fill((0, 6, 0), (n as i32 - 1, 6, 0), mx);
    m.fill((0, 7, 0), (n as i32 - 1, 7, 0), mf);
    (Demon::new(m), mf, mx)
}

#[test]
fn cold_matter_never_moves() {
    let (f, x, s) = fuel();
    let (mut d, _, _) = rod(16, &f, &x, &s);
    let before = d.digest();
    for _ in 0..600 {
        d.step();
        assert_eq!(d.active(), 0);
    }
    // Only the tick phase differs after 600 = 100 · 6 ticks: the state is identical.
    assert_eq!(d.digest(), before);
    assert_eq!(d.reactions(), 0);
}

#[test]
fn fuel_ignites_at_a_sharp_temperature_set_by_its_holding() {
    let (f, x, _) = fuel();
    let mut m = Matter::new(Grid::new(2, 1, 1));
    let (mf, mx) = (m.intern(&f), m.intern(&x));
    m.set(0, mf);
    m.set(1, mx);
    let need = ignition(&f, &x);
    let held: Vec<i64> = f.holding().iter().map(|&h| i64::from(h) / super::UNIT + 1).collect();
    println!("fuel holdings + 1 (quanta): {held:?}; ignition {need}; best move {:?}", best(&f, &x));
    assert!(held.contains(&i64::from(need)), "the barrier is the holding of the leaving occurrence");
    // Both cells at one temperature T, so conduction between them is zero.
    let run = |t: u32| {
        let mut d = Demon::new(m.clone());
        d.poke(0, t as i32);
        d.poke(1, t as i32);
        let (b0, e0) = (d.energy().1, d.energy().2);
        let mut most = 0;
        for _ in 0..1200 {
            d.step();
            most = most.max(d.energy().1 - b0);
            assert_eq!(d.energy().2, e0);
        }
        (d.reactions(), most)
    };
    for t in [0, 1, need / 2, need - 1] {
        assert_eq!(run(t).0, 0, "below the ignition temperature nothing happens, forever (T = {t})");
    }
    let (moves, _) = run(need);
    println!("at T = {need}: {moves} moves in 1200 ticks");
    assert!(moves >= 1);
    let hot = best(&f, &x).1[0].max(best(&f, &x).1[1]);
    let (moves, most) = run(hot);
    println!("at T = {hot} (the best move's need): {moves} moves, binding rose by up to {most} quanta");
    assert!(most > i64::from(hot), "the fuel releases more than its ignition heat");
}

#[test]
fn a_combustion_front_crosses_the_rod_at_a_steady_speed() {
    let (f, x, s) = fuel();
    let n = 48u32;
    let (mut d, mf, _) = rod(n, &f, &x, &s);
    let need = ignition(&f, &x);
    let top = |c: u32| (7 * n + c) as usize;
    d.poke(top(0), 3 * need as i32);
    d.poke(top(0) - n as usize, 3 * need as i32);
    let (e0, occ0) = (d.energy().2, d.matter().occurrences());
    let mut lit = vec![u32::MAX; n as usize];
    let mut t = 0u32;
    while lit[n as usize - 1] == u32::MAX && t < 4000 {
        d.step();
        t += 1;
        for c in 0..n {
            if lit[c as usize] == u32::MAX && d.matter().get(top(c)) != mf {
                lit[c as usize] = t;
            }
        }
        if t.is_multiple_of(150) {
            println!("t {t}, palette {}:\n{}", d.matter().palette_len(), show::slice(&d, 0));
        }
        assert_ledger(&d, e0, occ0);
    }
    println!("ignition tick per column: {lit:?}");
    assert!(lit.iter().all(|&v| v != u32::MAX), "the front reaches the far end");
    let speed = |a: usize, b: usize| (b - a) as f64 / f64::from(lit[b] - lit[a]);
    let (first, second, whole) = (speed(8, 28), speed(28, 47), speed(8, 47));
    println!("front speed: {first:.4} then {second:.4}, overall {whole:.4} cells/tick; palette {}", d.matter().palette_len());
    assert!(first / second < 2.0 && second / first < 2.0, "a steady front, not a fizzle or a runaway");
}

#[test]
fn a_firebreak_stops_the_front_and_no_heat_crosses_it() {
    let (f, x, s) = fuel();
    let i = firebreak(&[&f, &x, &s]);
    let n = 40u32;
    let (d, mf, _) = rod(n, &f, &x, &s);
    let mut m = d.matter().clone();
    let mi = m.intern(&i);
    let wall = 20;
    m.fill((wall, 0, 0), (wall, 7, 0), mi);
    println!("firebreak self-bond {} bonds to F, X, S: {} {} {}", bond(&i, &i), bond(&i, &f), bond(&i, &x), bond(&i, &s));
    let mut d = Demon::new(m);
    let need = ignition(&f, &x);
    let top = |c: u32| (7 * n + c) as usize;
    d.poke(top(0), 3 * need as i32);
    d.poke(top(0) - n as usize, 3 * need as i32);
    let (e0, occ0) = (d.energy().2, d.matter().occurrences());
    let beyond = |d: &Demon| (0..d.matter().grid.len()).filter(|&c| c as u32 % n > wall as u32).map(|c| u64::from(d.heat(c))).sum::<u64>();
    for _ in 0..1500 {
        d.step();
        assert_eq!(beyond(&d), 0, "not one quantum crosses the firebreak");
        assert_ledger(&d, e0, occ0);
    }
    println!("{}", show::slice(&d, 0));
    let burnt = (0..n).filter(|&c| d.matter().get(top(c)) != mf).count();
    println!("burnt columns: {burnt} (the wall is column {wall})");
    assert!(burnt >= 15, "the near side burns");
    assert!((wall as u32 + 1..n).all(|c| d.matter().get(top(c)) == mf), "the far side never does");
}

#[test]
fn the_ledger_closes_exactly_over_thousands_of_ticks() {
    let (f, x, s) = fuel();
    let mut m = Matter::new(Grid::new(12, 6, 3));
    let (mf, mx, ms) = (m.intern(&f), m.intern(&x), m.intern(&s));
    m.fill((0, 0, 0), (11, 1, 2), ms);
    m.fill((0, 2, 0), (11, 2, 2), mx);
    m.fill((0, 3, 0), (11, 3, 2), mf);
    m.fill((0, 4, 0), (11, 4, 2), mx);
    // Row 5 stays void: hot matter may evaporate into it.
    let mut d = Demon::new(m);
    let need = ignition(&f, &x) as i32;
    let (e0, occ0) = (d.energy().2, d.matter().occurrences());
    let g = d.matter().grid;
    let mut pokes = 0;
    for t in 0..6000u32 {
        if t.is_multiple_of(1000) {
            // Heat a fuel cell, and chill another: both are work.
            d.poke(g.idx(t / 1000 * 2, 3, 1), 4 * need);
            d.poke(g.idx(11 - t / 1000, 3, 0), -need);
            pokes += 2;
        }
        d.step();
        assert_ledger(&d, e0, occ0);
    }
    let l = d.ledger();
    println!("ledger after 6000 ticks and {pokes} pokes: {:?}; palette {}", l.entries, d.matter().palette_len());
    assert!(d.reactions() > 50, "the box was busy, not idle");
    assert_eq!(l.get("energy"), Some(e0));
}

/// An ore M and a collector C that never conduct to each other (bond ≤ 0), whose every move is
/// uphill (selective transfer would never run it), but some move only asks heat of the ore.
fn ore() -> (Block, Block) {
    let m = find(40, 3, 8, 100_000, |b| is_cohesive(b) && cohesion(b) > 40);
    let c = find_seq(300, |c| {
        let ms = contact_moves(&m, c);
        bond(&m, c) <= 0 && !ms.is_empty() && ms[0].delta[0] + ms[0].delta[1] < 0 && ms.iter().any(|v| v.need[1] == 0)
    });
    (m, c)
}

#[test]
fn heat_drives_a_separation_that_cold_forbids_and_a_quench_keeps_it() {
    let (ore, col) = ore();
    let mut m = Matter::new(Grid::new(2, 1, 1));
    let (mo, mc) = (m.intern(&ore), m.intern(&col));
    m.set(0, mo);
    m.set(1, mc);
    let ms = contact_moves(&ore, &col);
    println!("ore | collector moves (δ, need): {:?}", ms.iter().map(|v| (v.delta, v.need)).collect::<Vec<_>>());
    let mut yields = Vec::new();
    for t in (0..=4000u32).step_by(250) {
        let mut d = Demon::new(m.clone());
        d.poke(0, t as i32);
        let (b0, e0) = (d.energy().1, d.energy().2);
        for _ in 0..600 {
            d.step();
        }
        let moved = i64::from(ore.len() as u8) - i64::from(d.matter().reading(d.matter().get(0)).len);
        let r = d.reactions();
        // Quench: take all heat out; nothing moves ever again.
        let snapshot = d.matter().cells().to_vec();
        for c in 0..2 {
            let q = d.heat(c) as i32;
            d.poke(c, -q);
        }
        for _ in 0..600 {
            d.step();
        }
        assert_eq!(d.matter().cells(), &snapshot[..], "the quenched products persist");
        assert_eq!(d.energy().2, e0);
        yields.push((t, moved, r, d.energy().1 - b0));
    }
    println!("(ore heat, occurrences left the ore, moves, binding change): {yields:?}");
    assert_eq!(yields[0].2, 0, "cold: nothing");
    assert!(yields.iter().any(|y| y.1 > 0 && y.3 < 0), "hot: an uphill separation, paid for by heat");
}

/// A pair that only reacts when both cells are hot: every move pushes an occurrence into a cell
/// that repels it, so the receiver pays too. They repel overall, so no heat passes between them.
fn lock() -> (Block, Block) {
    let p = find(50, 3, 8, 100_000, |b| is_cohesive(b) && cohesion(b) > 40);
    let q = find_seq(500, |q| {
        let ms = contact_moves(&p, q);
        bond(&p, q) <= 0 && !ms.is_empty() && ms.iter().all(|v| v.need[0] > 0 && v.need[1] > 0)
    });
    (p, q)
}

#[test]
fn a_two_key_lock_is_a_thermal_and_gate() {
    let (p, q) = lock();
    let mut m = Matter::new(Grid::new(2, 1, 1));
    let (mp, mq) = (m.intern(&p), m.intern(&q));
    m.set(0, mp);
    m.set(1, mq);
    let ms = contact_moves(&p, &q);
    let hot = ms.iter().map(|v| v.need[0].max(v.need[1])).max().unwrap() as i32;
    println!("lock moves (δ, need): {:?}; input heat {hot}", ms.iter().map(|v| (v.delta, v.need)).collect::<Vec<_>>());
    let mut table = Vec::new();
    for (a, b) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let mut d = Demon::new(m.clone());
        d.poke(0, a * hot);
        d.poke(1, b * hot);
        for _ in 0..600 {
            d.step();
        }
        table.push(((a, b), d.reactions()));
    }
    println!("truth table (inputs, moves): {table:?}");
    assert!(table[..3].iter().all(|&(_, r)| r == 0));
    assert!(table[3].1 > 0);
}

/// Ticks until the state first repeats, and the period of the cycle it then runs forever.
fn cycle(d: &mut Demon, max: u32) -> Option<(u32, u32)> {
    let mut seen = std::collections::HashMap::new();
    for t in 0..max {
        if let Some(t0) = seen.insert(d.digest(), t) {
            return Some((t0, t - t0));
        }
        d.step();
    }
    None
}


#[test]
fn a_closed_box_never_halts_it_settles_into_a_cycle() {
    // Three cells in a closed line: oxidizer, fuel, oxidizer, the left one heated. Energy cannot
    // leave, the state space is finite and the law deterministic, so the box must end in a cycle;
    // descent would end in a fixed point (period 6, the phase schedule alone).
    let (f, x, _) = fuel();
    let mut runs = Vec::new();
    for heat in [2000, 4000, 9000] {
        let mut m = Matter::new(Grid::new(3, 1, 1));
        let (mf, mx) = (m.intern(&f), m.intern(&x));
        m.set(0, mx);
        m.set(1, mf);
        m.set(2, mx);
        let mut d = Demon::new(m);
        d.poke(0, heat);
        let e0 = d.energy().2;
        let (transient, period) = cycle(&mut d, 100_000).expect("a finite deterministic box cycles");
        let before = d.reactions();
        let mut trace = String::new();
        for _ in 0..period {
            trace.push_str(&format!("{:>6} {:>6} {:>6}\n", d.heat(0), d.heat(1), d.heat(2)));
            d.step();
        }
        assert_eq!(d.energy().2, e0);
        let moves = d.reactions() - before;
        if heat == 2000 {
            println!("heat per cell over one period:\n{trace}");
        }
        runs.push((heat, transient, period, moves));
    }
    println!("(heat put in, transient ticks, period ticks, moves per period): {runs:?}");
    for &(_, _, period, moves) in &runs {
        assert!(period > 6 && moves > 0, "it never comes to rest");
    }
    assert!(runs.iter().any(|r| r.2 > 12), "a clock slower than the schedule's flicker");
}

#[test]
fn a_lock_flicker_keeps_every_cell_energy_exact() {
    // In the lock no face conducts, so each cell's q − binding may change only by conduction:
    // never. Chemistry alone must leave both cell energies exactly as they were.
    let (p, q) = lock();
    let mut m = Matter::new(Grid::new(2, 1, 1));
    let (mp, mq) = (m.intern(&p), m.intern(&q));
    m.set(0, mp);
    m.set(1, mq);
    let mut d = Demon::new(m);
    d.poke(0, 9000);
    d.poke(1, 9000);
    let cell = |d: &Demon, c: usize| i64::from(d.heat(c)) - d.binding(d.matter().get(c));
    let (c0, c1) = (cell(&d, 0), cell(&d, 1));
    for _ in 0..600 {
        d.step();
        assert_eq!((cell(&d, 0), cell(&d, 1)), (c0, c1));
    }
    let (transient, period) = cycle(&mut d, 10_000).unwrap();
    println!("{} moves; cycle after {transient} ticks with period {period}", d.reactions());
    assert!(d.reactions() > 0);
}

#[test]
fn preheating_speeds_the_front_and_past_ignition_it_explodes() {
    let (f, x, s) = fuel();
    let n = 32u32;
    let need = ignition(&f, &x);
    let mut out = Vec::new();
    for pre in [0, need / 4, need / 2, 3 * need / 4, need] {
        let (mut d, mf, _) = rod(n, &f, &x, &s);
        for c in 0..d.matter().grid.len() {
            d.poke(c, pre as i32);
        }
        let top = |c: u32| (7 * n + c) as usize;
        d.poke(top(0), 3 * need as i32);
        d.poke(top(0) - n as usize, 3 * need as i32);
        let mut t = 0;
        let mut half = None;
        while t < 3000 {
            d.step();
            t += 1;
            let burnt = (0..n).filter(|&c| d.matter().get(top(c)) != mf).count() as u32;
            if half.is_none() && burnt >= n / 2 {
                half = Some(t);
            }
            if burnt == n {
                break;
            }
        }
        out.push((pre, half, t));
    }
    println!("(preheat, ticks to burn half, ticks to burn all): {out:?}");
    let all: Vec<u32> = out.iter().map(|o| o.2).collect();
    assert!(all.windows(2).all(|w| w[1] <= w[0]), "hotter is never slower");
    assert!(all[4] <= 3, "past the ignition temperature the whole rod goes in one sweep");
}

#[test]
fn a_hot_atom_flies_and_a_cold_one_sits() {
    let lone = find_with(9, 1, 1000, |_| true);
    let mut m = Matter::new(Grid::new(12, 12, 12));
    let a = m.intern(&lone);
    let g = m.grid;
    m.set(g.idx(2, 2, 2), a);
    let mut cold = Demon::new(m.clone());
    let mut hot = Demon::new(m);
    hot.poke(g.idx(2, 2, 2), 1);
    let at = |d: &Demon| g.pos(d.matter().cells().iter().position(|&c| c == a).unwrap());
    let mut path = Vec::new();
    for _ in 0..60 {
        cold.step();
        hot.step();
        path.push(at(&hot));
    }
    println!("hot atom path (every 6 ticks): {:?}", path.iter().step_by(6).collect::<Vec<_>>());
    assert_eq!(at(&cold), (2, 2, 2));
    assert_eq!(hot.heat(g.idx(at(&hot).0, at(&hot).1, at(&hot).2)), 1, "its heat travels with it");
    // It moves one cell per axis every three ticks until a wall turns it back.
    assert_eq!(path[5], (4, 4, 4));
}

#[test]
fn two_peers_compute_the_same_bytes() {
    let (f, x, s) = fuel();
    let run = || {
        let (mut d, _, _) = rod(24, &f, &x, &s);
        let need = ignition(&f, &x) as i32;
        d.poke(7 * 24, 3 * need);
        d.poke(6 * 24, 3 * need);
        for _ in 0..500 {
            d.step();
        }
        d.digest()
    };
    assert_eq!(run(), run());
}

/// A pair where heat in the second cell makes a better move affordable that pre-empts the move
/// the first cell's heat alone would make: inhibition from ranking alone. No conduction between.
fn inhibitor() -> (Block, Block) {
    let p = find(60, 3, 8, 100_000, |b| is_cohesive(b) && cohesion(b) > 40);
    let q = find_seq(700, |q| {
        let ms = contact_moves(&p, q);
        bond(&p, q) <= 0
            && ms.len() >= 2
            && ms[0].need[0] > 0
            && ms[0].need[1] > 1
            && ms[1..].iter().any(|v| v.need[1] == 0 && v.need[0] >= ms[0].need[0])
    });
    (p, q)
}

#[test]
fn inhibition_by_preemption_fails_the_state_is_reached_anyway() {
    let (p, q) = inhibitor();
    let mut m = Matter::new(Grid::new(2, 1, 1));
    let (mp, mq) = (m.intern(&p), m.intern(&q));
    m.set(0, mp);
    m.set(1, mq);
    let ms = contact_moves(&p, &q);
    println!("moves (δ, need): {:?}", ms.iter().map(|v| (v.delta, v.need)).collect::<Vec<_>>());
    let a = ms.iter().map(|v| v.need[0]).max().unwrap();
    let b = ms.iter().map(|v| v.need[1]).max().unwrap();
    let mut probe = Demon::new(m.clone());
    let alone = probe.outcome(mp, mq, [a, 0]).expect("the first cell's heat alone moves");
    let both = probe.outcome(mp, mq, [a, b]).expect("both hot moves");
    assert_ne!((alone.0, alone.1), (both.0, both.1), "a different move wins when both are hot");
    let mut table = Vec::new();
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let mut d = Demon::new(m.clone());
        d.poke(0, (x * a) as i32);
        d.poke(1, (y * b) as i32);
        let mut fired = false;
        for _ in 0..600 {
            d.step();
            fired |= (d.matter().get(0), d.matter().get(1)) == (alone.0, alone.1);
        }
        table.push(((x, y), fired));
    }
    println!("did the first cell's own move ever happen? {table:?}");
    assert!(table[1].1 && !table[0].1);
    // A negative result: heat in the second cell changes which move goes first, but the pre-empted
    // state is reached a few moves later anyway, so this is not P AND NOT Q.
    assert!(table[3].1, "measured: the inhibited state is reached anyway");
}

#[test]
fn hot_matter_keeps_minting_configurations() {
    // The cost of a thermal world: while matter is hot it keeps visiting new compositions, and each
    // one is a new palette entry. Measured on the burning rod after the front has passed.
    let (f, x, s) = fuel();
    let n = 16u32;
    let (mut d, _, _) = rod(n, &f, &x, &s);
    let need = ignition(&f, &x) as i32;
    d.poke((7 * n) as usize, 3 * need);
    d.poke((6 * n) as usize, 3 * need);
    let mut seen = Vec::new();
    for t in 1..=2000u32 {
        d.step();
        if t.is_multiple_of(250) {
            seen.push((t, d.matter().palette_len(), d.active()));
        }
    }
    println!("(tick, palette, active cells): {seen:?}");
    assert!(seen[7].1 > seen[3].1 + 100, "still minting new configurations long after ignition");
}
