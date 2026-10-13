//! The experiments. Each builds real matter with `pwc_law_core::search`, runs the law, prints what
//! it saw (run with `--nocapture`) and asserts the measured phenomenon.

use pwc_law_core::search::{find, find_with, is_cohesive, Stream};
use pwc_law_core::show;
use pwc_law_core::{Candidate, Grid, Mat, Matter};

use super::*;
use crate::material::{probe_response, Block, Law};
use crate::rule::{self, Collision, Reflection, Rule, Sym};

fn light(b: &Block) -> i32 {
    probe_response(b, Law::current().probes.light)
}

/// An opaque, cohesive configuration: a wall.
fn opaque() -> Block {
    find(1, 3, 12, 10_000, |b| is_cohesive(b) && light(b) <= CLEAR)
}

/// A pure configuration that reads as clear: open to particles.
fn clear() -> Block {
    find_with(2, 8, 100_000, |b| light(b) > CLEAR)
}

fn idx(c: &Margolus, x: u32, y: u32, z: u32) -> usize {
    c.matter().grid.idx(x, y, z)
}

/// Fill each open cell of an inclusive box with a particle with probability `permille`/1000 (a fixed
/// stream: an initial condition, not part of the law).
fn sprinkle(c: &mut Margolus, lo: (u32, u32, u32), hi: (u32, u32, u32), permille: u64, seed: u64) -> i64 {
    let mut s = Stream::new(seed);
    let mut n = 0;
    for z in lo.2..=hi.2 {
        for y in lo.1..=hi.1 {
            for x in lo.0..=hi.0 {
                if s.below(1000) < permille && c.put(idx(c, x, y, z)) {
                    n += 1;
                }
            }
        }
    }
    n
}

/// Positions of every particle.
fn positions(c: &Margolus) -> Vec<(u32, u32, u32)> {
    let g = c.matter().grid;
    (0..g.len()).filter(|&i| c.has(i)).map(|i| g.pos(i)).collect()
}

/// Cell-by-cell reference of one tick, straight from the definition (outside the box is wall).
fn reference_step(c: &Margolus) -> Vec<bool> {
    let g = c.matter().grid;
    let o = c.phase() as i32;
    let table = rule::table(c.rule());
    let mut next = vec![false; g.len()];
    let starts = |n: u32| (-o..n as i32).step_by(2).collect::<Vec<_>>();
    let corner_at = |x0: i32, y0: i32, z0: i32, k: u8| (x0 + (k & 1) as i32, y0 + ((k >> 1) & 1) as i32, z0 + (k >> 2) as i32);
    for &z0 in &starts(g.nz) {
        for &y0 in &starts(g.ny) {
            for &x0 in &starts(g.nx) {
                let (mut s, mut w, mut cl) = (0u8, 0u8, false);
                for k in 0..8u8 {
                    let p = corner_at(x0, y0, z0, k);
                    match g.at(p.0, p.1, p.2) {
                        Some(i) if c.is_open(i) => {
                            s |= u8::from(c.has(i)) << k;
                            cl |= c.matter().get(i) != VOID;
                        }
                        _ => w |= 1 << k,
                    }
                }
                let t = if w == 0 && cl { rule::flip(s, 7) } else { table[(w as usize) << 8 | s as usize] };
                for k in (0..8u8).filter(|k| (t >> k) & 1 == 1) {
                    let p = corner_at(x0, y0, z0, k);
                    next[g.at(p.0, p.1, p.2).expect("particles stay in the box")] = true;
                }
            }
        }
    }
    next
}

/// Shannon entropy (nats) of particle counts over a coarse grid of `cell`³ boxes.
fn coarse_entropy(c: &Margolus, cell: u32) -> f64 {
    let mut counts = std::collections::BTreeMap::new();
    for (x, y, z) in positions(c) {
        *counts.entry((x / cell, y / cell, z / cell)).or_insert(0u64) += 1;
    }
    let n: u64 = counts.values().sum();
    counts.values().map(|&k| k as f64 / n as f64).map(|p| -p * p.ln()).sum()
}

#[test]
fn every_block_map_is_a_bijection_and_its_own_inverse() {
    for r in Rule::ALL {
        let t = rule::table(r);
        let mut states = 0;
        for w in 0..256usize {
            let m = rule::mirror_axes(w as u8);
            for s in (0..256usize).filter(|s| s & w == 0) {
                states += 1;
                let o = t[w << 8 | s] as usize;
                assert_eq!(o & w, 0, "a particle entered a wall");
                assert_eq!(o.count_ones(), s.count_ones(), "particle number");
                assert_eq!(t[w << 8 | o] as usize, s, "not an involution at w={w:#x} s={s:#x}");
                let (pin, pout) = (rule::momentum(s as u8), rule::momentum(rule::flip(o as u8, 7)));
                if w == 0 && r.collision == Collision::Derived {
                    assert_eq!(pin, pout, "momentum in empty space");
                } else if w != 0 && r.reflection == Reflection::Mirror {
                    for a in (0..3).filter(|a| (m >> a) & 1 == 1) {
                        assert_eq!(pin[a], pout[a], "momentum along a mirror axis");
                    }
                }
            }
        }
        assert_eq!(states, 6561, "3^8 (wall, particle) block states");
        println!("{r:?}: all {states} open block states map one-to-one, and twice is identity");
    }
    for s in 0..=255u8 {
        assert_eq!(rule::flip(rule::flip(s, 7), 7), s);
    }
    // Mirror axes: empty space all three, a flat wall (the x = 1 face) two, an inside edge one, a lone
    // wall corner or a diagonal pair of wall corners none (so diagonal gaps never leak).
    assert_eq!(rule::mirror_axes(0), 0b111);
    assert_eq!(rule::mirror_axes(0b1010_1010), 0b110);
    assert_eq!(rule::mirror_axes(0b1000_1000), 0b100);
    assert_eq!(rule::mirror_axes(0b1000_0000), 0);
    assert_eq!(rule::mirror_axes(0b1000_0001), 0);
    let g = rule::collision();
    let colliding = (0..256).filter(|&s| g[s] != s as u8).count();
    println!("states that collide in empty space: {colliding} of 256");
    assert_eq!(colliding, 80);
}

/// The fixed-point-free bijections of `members` (no symmetry asked).
fn all_derangements(members: &[u8]) -> Vec<Vec<u8>> {
    let identity = rule::symmetries().into_iter().find(|g| g.map == [0, 1, 2, 3, 4, 5, 6, 7]).expect("identity");
    rule::commuting_derangements(members, &[identity])
}

/// A symmetry as the image of each velocity axis: `M e_a = (v(g·0) − v(g·(1<<a))) / 2`.
fn describe(g: &Sym) -> String {
    let names = ["x", "y", "z"];
    (0..3u8)
        .map(|a| {
            let v: Vec<i32> = (0..3).map(|k| (rule::vel(g.map[0], k) - rule::vel(g.map[1 << a], k)) / 2).collect();
            let k = (0..3).find(|&k| v[k] != 0).expect("an axis");
            format!("{}→{}{}", names[a as usize], if v[k] > 0 { "+" } else { "−" }, names[k])
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn the_collision_is_forced_by_the_symmetry_of_the_cube() {
    let group = rule::symmetries();
    let rotations: Vec<Sym> = group.iter().copied().filter(|g| g.proper).collect();
    assert_eq!(group.len(), 48);
    assert_eq!(rotations.len(), 24);
    println!(" n  momentum      states  isotropic  chiral-only");
    let mut deflecting = [0usize; 9];
    for ((n, p), members) in rule::classes() {
        if members.len() < 2 {
            continue;
        }
        let full = rule::commuting_derangements(&members, &rule::stabilizer(&group, members[0])).len();
        let chiral = rule::commuting_derangements(&members, &rule::stabilizer(&rotations, members[0])).len();
        println!("{n:2}  {p:?}  {:6}  {full:9}  {chiral:11}", members.len());
        assert!(full <= 1, "the derivation is unique wherever it exists");
        if full == 1 {
            deflecting[n as usize] += 1;
        }
        if n == 2 && p == [0, 0, 0] {
            assert_eq!(full, 0, "no isotropic head-on deflection");
            assert_eq!(chiral, 0, "not even a chiral one");
        }
        if n % 2 == 1 {
            assert_eq!(chiral, 0, "odd numbers never collide");
        }
        if n == 4 && members.len() == 4 {
            assert_eq!(chiral, 3, "a handed medium would not pick one collision");
        }
    }
    println!("classes with a forced collision by particle number: {deflecting:?}");
    assert_eq!(deflecting, [0, 0, 6, 0, 19, 0, 6, 0, 0]);

    // What a scalar property of matter could do to a lone particle: only maps that commute with every
    // symmetry are allowed, and the only one that moves anything is reversing it.
    let singles: Vec<u8> = (0..8).map(|c| 1u8 << c).collect();
    let lone = rule::commuting_derangements(&singles, &group);
    assert_eq!(lone, vec![singles.iter().map(|&s| rule::flip(s, 7)).collect::<Vec<_>>()]);
    println!("isotropic maps of a lone particle: keep flying, or the one derangement, reverse");

    // The derived collision is isotropic, its own inverse and symmetric under particle ↔ hole.
    let g = rule::collision();
    // Glancing pair (+,+,+), (−,−,+) leaves as (−,+,+), (+,−,+); the two tetrahedra swap; two head-on
    // pairs leave on the other two diagonals.
    assert_eq!(g[0b0000_1001], 0b0000_0110);
    assert_eq!(g[0b0110_1001], 0b1001_0110);
    assert_eq!(g[0b1100_0011], 0b0011_1100);
    for s in 0..=255u8 {
        assert_eq!(g[g[s as usize] as usize], s);
        assert_eq!(g[!s as usize], !g[s as usize]);
        for h in &group {
            assert_eq!(g[h.act(s) as usize], h.act(g[s as usize]));
        }
    }

    // The brief's head-on deflection: every candidate picks an axis, and two thirds pick a hand.
    let head_on: Vec<u8> = (0..4u8).map(|c| 1 << c | 1 << (c ^ 7)).collect();
    let pos = |s: u8| head_on.iter().position(|&m| m == s).expect("pairs map to pairs");
    let ds = all_derangements(&head_on);
    assert_eq!(ds.len(), 9);
    let (mut half, mut quarter) = (0, 0);
    for d in &ds {
        let realize: Vec<&Sym> = rotations.iter().filter(|g| head_on.iter().zip(d).all(|(&s, &t)| g.act(s) == t)).collect();
        assert_eq!(realize.len(), 1, "each deflection is one rotation of the pair");
        let commuting = group.iter().filter(|h| head_on.iter().zip(d).all(|(&s, &t)| h.act(t) == d[pos(h.act(s))])).count();
        let involution = head_on.iter().zip(d).all(|(&s, &t)| d[pos(t)] == s);
        if involution {
            half += 1;
        } else {
            quarter += 1;
        }
        println!(
            "head-on deflection = rotation [{}]: {}, commutes with {commuting} of 48 symmetries",
            describe(realize[0]),
            if involution { "180° turn, achiral" } else { "90° turn, chiral" }
        );
        assert_eq!(commuting, if involution { 16 } else { 8 });
    }
    assert_eq!((half, quarter), (3, 6));
}

#[test]
fn the_word_engine_matches_the_cell_definition() {
    let wall = opaque();
    let glass = clear();
    let shapes = [(1, 1, 1), (2, 2, 2), (7, 5, 3), (64, 3, 2), (65, 4, 4), (130, 3, 5), (63, 6, 1)];
    for (k, &(nx, ny, nz)) in shapes.iter().enumerate() {
        let mut m = Matter::new(Grid::new(nx, ny, nz));
        let w = m.intern(&wall);
        let gl = m.intern(&glass);
        let mut s = Stream::new(100 + k as u64);
        for i in 0..m.grid.len() {
            match s.below(10) {
                0 => m.set(i, w),
                1 => m.set(i, gl),
                _ => {}
            }
        }
        for r in Rule::ALL {
            let mut c = Margolus::with_rule(m.clone(), r);
            sprinkle(&mut c, (0, 0, 0), (nx - 1, ny - 1, nz - 1), 450, 7 + k as u64);
            let n = c.particles();
            for _ in 0..40 {
                let want = reference_step(&c);
                c.step();
                let got: Vec<bool> = (0..m.grid.len()).map(|i| c.has(i)).collect();
                assert_eq!(got, want, "{nx}×{ny}×{nz} {r:?} at tick {}", c.ticks());
                assert_eq!(c.particles(), n);
            }
        }
    }
    println!("word-parallel engine = cell-by-cell definition on {} box shapes, all four block rules, 40 ticks", shapes.len());
}

#[test]
fn particles_are_conserved_and_momentum_changes_only_at_walls() {
    let mut c = Margolus::new(Matter::new(Grid::new(48, 48, 48)));
    let n = sprinkle(&mut c, (16, 16, 16), (31, 31, 31), 400, 5);
    let p0 = c.momentum();
    let mut collisions = 0;
    for _ in 0..14 {
        c.step();
        assert_eq!(c.wall_hits(), 0);
        assert_eq!(c.momentum(), p0, "momentum is exact in empty space");
        collisions += c.collisions();
    }
    println!("{n} particles, momentum {p0:?} exact over 14 ticks of free flight with {collisions} collisions");
    assert!(collisions > 500);
    let mut hits = 0;
    let mut twin = c.clone();
    for _ in 0..3000 {
        let p = c.momentum();
        c.step();
        let q = c.momentum();
        let moved: i64 = (0..3).map(|a| (q[a] - p[a]).abs()).sum();
        assert!(moved <= 2 * c.wall_hits(), "momentum changed by {moved} with {} wall hits", c.wall_hits());
        assert_eq!(c.particles(), n);
        assert_eq!(c.ledger().get("particles"), Some(n));
        hits += c.wall_hits();
        twin.step();
    }
    assert_eq!(c.digest(), twin.digest(), "deterministic");
    println!("3000 more ticks in the closed box: {n} particles throughout, {hits} wall hits, momentum now {:?}", c.momentum());
}

#[test]
fn a_gas_expansion_unscrambles_exactly() {
    let mut m = Matter::new(Grid::new(32, 32, 32));
    let wall = m.intern(&opaque());
    m.fill((16, 0, 0), (16, 31, 31), wall);
    m.fill((16, 13, 13), (16, 18, 18), VOID);
    let mut c = Margolus::new(m);
    let n = sprinkle(&mut c, (0, 0, 0), (15, 31, 31), 400, 9);
    let start = c.digest();
    let s0 = coarse_entropy(&c, 8);
    println!("tick 0, z=15:\n{}", show::slice(&c, 15));
    let mut entropy = vec![(0, s0)];
    for t in 1..=600 {
        c.step();
        if t % 150 == 0 {
            entropy.push((t, coarse_entropy(&c, 8)));
        }
    }
    let right = positions(&c).iter().filter(|p| p.0 > 16).count();
    println!("tick 600, z=15:\n{}", show::slice(&c, 15));
    println!("coarse entropy (nats, 8³ cells): {entropy:.3?}; {right} of {n} particles crossed the hole");
    assert!(right > n as usize / 10);
    assert!(entropy.last().expect("measured").1 > s0 + 0.3);
    let spread = c.clone();

    for _ in 0..600 {
        c.unstep();
    }
    assert_eq!(c.digest(), start, "unstep 600 restores every bit");
    println!("unstep ×600: identical to tick 0 (digest {start:#018x}), entropy {:.4}", coarse_entropy(&c, 8));

    // Reverse every velocity instead and run forward: the egg unscrambles by itself.
    let mut c = spread;
    c.reverse();
    for _ in 0..600 {
        c.step();
    }
    c.reverse();
    assert_eq!(c.digest(), start, "reverse, 600 forward, reverse = tick 0");
    println!("reverse + 600 ticks forward: back in the left chamber, entropy {:.4}", coarse_entropy(&c, 8));
}

#[test]
fn effusion_through_a_hole_has_a_measured_rate() {
    let mut m = Matter::new(Grid::new(49, 24, 24));
    let wall = m.intern(&opaque());
    m.fill((24, 0, 0), (24, 23, 23), wall);
    m.fill((24, 10, 10), (24, 13, 13), VOID);
    let area = 16.0;
    let volume = 24.0 * 24.0 * 24.0;
    let mut c = Margolus::new(m);
    let n = sprinkle(&mut c, (0, 0, 0), (23, 23, 23), 300, 3) as f64;
    let rho = n / volume;
    let left = |c: &Margolus| positions(c).iter().filter(|p| p.0 < 24).count() as f64;
    let mut early = 0.0;
    let mut log = Vec::new();
    for t in 1..=8000 {
        c.step();
        if t == 40 {
            early = (n - left(&c)) / 40.0;
        }
        if t % 100 == 0 {
            log.push((t as f64, 2.0 * left(&c) - n));
        }
    }
    // Free-molecular prediction: half the particles at the hole face move toward it, one cell a tick.
    let ideal = rho * area / 2.0;
    let kappa = early / ideal;
    // Two equal chambers: d(ΔN)/dt = −(κ·A/V)·ΔN.
    let tau_pred = volume / (kappa * area);
    let fit: Vec<(f64, f64)> = log.iter().filter(|&&(_, d)| d > 0.15 * n).map(|&(t, d)| (t, d.ln())).collect();
    let k = fit.len() as f64;
    let (st, sy) = (fit.iter().map(|p| p.0).sum::<f64>(), fit.iter().map(|p| p.1).sum::<f64>());
    let stt: f64 = fit.iter().map(|p| p.0 * p.0).sum();
    let sty: f64 = fit.iter().map(|p| p.0 * p.1).sum();
    let tau = -1.0 / ((k * sty - st * sy) / (k * stt - st * st));
    let last = log.last().expect("logged").1;
    println!(
        "density {rho:.3}, hole {area} cells: early flux {early:.3}/tick vs ideal ρA/2 = {ideal:.3} \
         (transmission κ = {kappa:.3}); imbalance decays with τ = {tau:.0} ticks vs V/(κA) = {tau_pred:.0} \
         ({} points); imbalance at 8000 ticks {last:.0} of {n:.0}",
        fit.len()
    );
    assert!((tau / tau_pred - 1.0).abs() < 0.25, "relaxation follows effusion");
    assert!(last.abs() < 4.0 * n.sqrt(), "the chambers equalize");
}

#[test]
fn pressure_is_proportional_to_density() {
    let side = 20u32;
    let area = 6.0 * f64::from(side * side);
    let mut ratios = Vec::new();
    for permille in [25, 50, 100, 200, 300, 400, 500, 600] {
        let mut c = Margolus::new(Matter::new(Grid::new(side, side, side)));
        let n = sprinkle(&mut c, (0, 0, 0), (side - 1, side - 1, side - 1), permille, 11);
        for _ in 0..200 {
            c.step();
        }
        let mut hits = 0;
        let ticks = 2000;
        for _ in 0..ticks {
            c.step();
            hits += c.wall_hits();
        }
        let rho = n as f64 / f64::from(side * side * side);
        let p = hits as f64 / (f64::from(ticks) * area);
        println!("density {rho:.3}: pressure {p:.4} hits/tick/face cell, p/ρ = {:.4}", p / rho);
        ratios.push(p / rho);
    }
    let (lo, hi) = ratios.iter().fold((f64::MAX, 0.0f64), |(a, b), &r| (a.min(r), b.max(r)));
    println!("p/ρ spread {:.3}%", 100.0 * (hi / lo - 1.0));
    assert!(hi / lo < 1.0001, "an ideal gas: p ∝ ρ, exactly");
    for permille in [50, 300, 600] {
        let literal = Rule { reflection: Reflection::Mirror, ..Rule::LITERAL };
        let mut c = Margolus::with_rule(Matter::new(Grid::new(side, side, side)), literal);
        let n = sprinkle(&mut c, (0, 0, 0), (side - 1, side - 1, side - 1), permille, 11);
        let mut hits = 0;
        for _ in 0..2000 {
            c.step();
            hits += c.wall_hits();
        }
        let rho = n as f64 / f64::from(side * side * side);
        println!("literal collisions, mirror walls, density {rho:.3}: p/ρ = {:.4}", hits as f64 / (2000.0 * area) / rho);
    }
}

#[test]
fn a_particle_in_a_box_returns_forever() {
    let (lx, ly, lz) = (3u32, 5u32, 7u32);
    let mut m = Matter::new(Grid::new(lx + 2, ly + 2, lz + 2));
    let wall = m.intern(&opaque());
    m.fill((0, 0, 0), (lx as i32 + 1, ly as i32 + 1, lz as i32 + 1), wall);
    m.fill((1, 1, 1), (lx as i32, ly as i32, lz as i32), VOID);
    let gcd = |mut a: u64, mut b: u64| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let lcm = |a: u64, b: u64| a / gcd(a, b) * b;
    let predicted = lcm(lcm(2 * u64::from(lx), 2 * u64::from(ly)), 2 * u64::from(lz));
    for start in [(1, 1, 1), (2, 3, 4), (3, 5, 2)] {
        let mut c = Margolus::new(m.clone());
        assert!(c.put(idx(&c, start.0, start.1, start.2)));
        let d0 = c.digest();
        let mut period = 0;
        for t in 1..=10_000u64 {
            c.step();
            assert_eq!(c.particles(), 1);
            if c.digest() == d0 {
                period = t;
                break;
            }
        }
        println!("open box {lx}×{ly}×{lz}, particle from {start:?}: period {period} (lcm(2Lx,2Ly,2Lz) = {predicted})");
        assert_eq!(period, predicted);
        for _ in 0..100 * period {
            c.step();
        }
        assert_eq!(c.digest(), d0, "still there after 100 more periods");
    }
    let mut empty = Margolus::new(m);
    for _ in 0..1000 {
        empty.step();
    }
    assert_eq!(empty.particles(), 0, "an empty box stays empty: one stored bit");
}

#[test]
fn a_mirror_routes_a_signal_and_bounce_walls_only_return_it() {
    // A screen hides a room (x ≤ 6, y ≥ 13) from the source; a mirror plane at x = 20 is the way in.
    let mut m = Matter::new(Grid::new(24, 24, 24));
    let wall = m.intern(&opaque());
    m.fill((0, 12, 0), (13, 12, 23), wall);
    m.fill((20, 0, 0), (20, 23, 23), wall);
    let in_room = |p: &(u32, u32, u32)| p.0 <= 6 && p.1 >= 13;
    let mut arrival = Vec::new();
    for reflection in [Reflection::Mirror, Reflection::Bounce] {
        let mut c = Margolus::with_rule(m.clone(), Rule { reflection, ..Rule::LAW });
        let source = idx(&c, 6, 2, 4);
        assert!(c.put(source));
        assert_eq!(c.velocity(source), [1, 1, 1]);
        let mut first = None;
        let mut trace = String::new();
        for t in 1..=2000 {
            c.step();
            let p = positions(&c)[0];
            if t <= 40 {
                trace.push_str(&format!("{:?} ", (p.0, p.1)));
            }
            if first.is_none() && in_room(&p) {
                first = Some(t);
                println!("{reflection:?}: entered the room at tick {t}, slice z={}:\n{}", p.2, show::slice(&c, p.2));
            }
        }
        println!("{reflection:?} xy path: {trace}");
        arrival.push(first);
    }
    println!("first arrival in the hidden room: mirror {:?}, bounce {:?}", arrival[0], arrival[1]);
    assert_eq!(arrival[0], Some(27), "13 ticks to the mirror, y climbs to the ceiling and back down to 18: 21 + 5 moves and one tick turning");
    assert_eq!(arrival[1], None, "bounce-back walls send the signal home, forever");
}

#[test]
fn bounce_walls_freeze_a_thin_slab_and_mirrors_do_not() {
    // Two cells thick: every second partition touches the floor or the ceiling.
    let m = Matter::new(Grid::new(16, 16, 2));
    for reflection in [Reflection::Mirror, Reflection::Bounce] {
        let mut c = Margolus::with_rule(m.clone(), Rule { reflection, ..Rule::LAW });
        c.put(idx(&c, 2, 2, 0));
        let mut reach = 0;
        for _ in 0..200 {
            c.step();
            let p = positions(&c)[0];
            reach = reach.max((p.0 as i32 - 2).abs().max((p.1 as i32 - 2).abs()));
        }
        println!("{reflection:?}: farthest in-plane distance from the source in 200 ticks: {reach}");
        assert_eq!(reach, if reflection == Reflection::Mirror { 13 } else { 1 });
    }
}

/// Inputs A (velocity +,+,+) and B (velocity −,−,+) meet in one block at tick 8 if both are present.
fn gate(a: bool, b: bool, glass: bool) -> (Margolus, [bool; 4]) {
    let mut m = Matter::new(Grid::new(40, 40, 40));
    if glass {
        let g = m.intern(&clear());
        m.fill((9, 9, 9), (16, 16, 16), g);
    }
    let mut c = Margolus::new(m);
    if a {
        assert!(c.put(idx(&c, 4, 4, 4)));
        assert_eq!(c.velocity(idx(&c, 4, 4, 4)), [1, 1, 1]);
    }
    if b {
        assert!(c.put(idx(&c, 21, 21, 4)));
        assert_eq!(c.velocity(idx(&c, 21, 21, 4)), [-1, -1, 1]);
    }
    let mut glass_seen = false;
    for _ in 0..20 {
        c.step();
        glass_seen |= positions(&c).iter().any(|&(x, y, z)| c.matter().get(idx(&c, x, y, z)) != VOID);
    }
    assert_eq!(glass_seen, glass && (a || b), "particles fly through clear matter");
    // Straight A, straight B, and the two deflected outputs.
    let out = [(24, 24, 24), (1, 1, 24), (24, 1, 24), (1, 24, 24)].map(|(x, y, z)| c.has(idx(&c, x, y, z)));
    (c, out)
}

#[test]
fn the_collision_gate_computes_and() {
    println!(" A B | A·¬B ¬A·B A·B A·B");
    for (a, b) in [(false, false), (true, false), (false, true), (true, true)] {
        let (c, out) = gate(a, b, false);
        let bit = |v: bool| u8::from(v);
        println!(" {} {} |  {}    {}    {}   {}", bit(a), bit(b), bit(out[0]), bit(out[1]), bit(out[2]), bit(out[3]));
        assert_eq!(out, [a && !b, b && !a, a && b, a && b]);
        if a && b {
            println!("both inputs, z=24 after 20 ticks (outputs at the four corners):\n{}", show::slice(&c, 24));
        }
    }
}

#[test]
fn crossing_paths_pass_through_each_other() {
    // A (+,+,+) with C (+,−,+): adjacent in one block, a class of one state. A with D (−,−,−): head-on,
    // and no isotropic head-on deflection exists, so they pass.
    for (other, v, out) in [((4, 21, 4), [1, -1, 1], (24, 1, 24)), ((21, 21, 21), [-1, -1, -1], (1, 1, 1))] {
        let mut c = Margolus::new(Matter::new(Grid::new(40, 40, 40)));
        c.put(idx(&c, 4, 4, 4));
        c.put(idx(&c, other.0, other.1, other.2));
        assert_eq!(c.velocity(idx(&c, other.0, other.1, other.2)), v);
        let mut met = false;
        for _ in 0..20 {
            let p = positions(&c);
            let o = c.phase() as u32;
            let block = |q: (u32, u32, u32)| ((q.0 + o) / 2, (q.1 + o) / 2, (q.2 + o) / 2);
            met |= block(p[0]) == block(p[1]);
            c.step();
            assert_eq!(c.collisions(), 0);
        }
        assert!(met, "they shared a block");
        assert!(c.has(idx(&c, 24, 24, 24)) && c.has(idx(&c, out.0, out.1, out.2)));
        println!("velocity (1,1,1) and {v:?} shared a block and both flew on: a crossover");
    }
}

#[test]
fn clear_matter_turns_the_gate_into_a_crossover() {
    let (c, out) = gate(true, true, true);
    println!("both inputs, collision site inside clear matter, z=24:\n{}", show::slice(&c, 24));
    assert_eq!(out, [true, true, false, false], "no collision inside clear matter");
}

/// Mean squared distance of the particles from their centre, per axis.
fn spread(c: &Margolus) -> [f64; 3] {
    let p = positions(c);
    let n = p.len() as f64;
    let axis = |f: fn(&(u32, u32, u32)) -> u32| {
        let mean = p.iter().map(|q| f64::from(f(q))).sum::<f64>() / n;
        p.iter().map(|q| (f64::from(f(q)) - mean).powi(2)).sum::<f64>() / n
    };
    [axis(|q| q.0), axis(|q| q.1), axis(|q| q.2)]
}

/// A cloud in a large box after 32 ticks: its growth in spread per axis relative to the same cloud
/// with collisions switched off (clear matter everywhere), and how much its momentum moved.
fn cloud(rule: Rule, permille: u64) -> ([f64; 3], i64) {
    let mut m = Matter::new(Grid::new(96, 96, 96));
    let mut c = Margolus::with_rule(m.clone(), rule);
    let glass = m.intern(&clear());
    m.fill((0, 0, 0), (95, 95, 95), glass);
    let mut free = Margolus::with_rule(m, rule);
    sprinkle(&mut c, (40, 40, 40), (55, 55, 55), permille, 13);
    sprinkle(&mut free, (40, 40, 40), (55, 55, 55), permille, 13);
    let (s0, p0) = (spread(&c), c.momentum());
    for _ in 0..32 {
        c.step();
        free.step();
    }
    let (s, f, p) = (spread(&c), spread(&free), c.momentum());
    ([0, 1, 2].map(|a| (s[a] - s0[a]) / (f[a] - s0[a])), (0..3).map(|a| (p[a] - p0[a]).abs()).sum())
}

#[test]
fn collisions_are_invisible_in_every_projection() {
    // Momentum and number conserved per block mean each block sends exactly as many particles to each
    // x half as free flight would: the x-marginal of the gas evolves as if nothing ever collided.
    let mut m = Matter::new(Grid::new(64, 64, 64));
    let mut c = Margolus::new(m.clone());
    let glass = m.intern(&clear());
    m.fill((0, 0, 0), (63, 63, 63), glass);
    let mut free = Margolus::new(m);
    sprinkle(&mut c, (20, 20, 20), (43, 43, 43), 500, 17);
    sprinkle(&mut free, (20, 20, 20), (43, 43, 43), 500, 17);
    let marginals = |c: &Margolus| {
        let p = positions(c);
        let mut out = [vec![0u32; 64], vec![0u32; 64], vec![0u32; 64]];
        for q in p {
            out[0][q.0 as usize] += 1;
            out[1][q.1 as usize] += 1;
            out[2][q.2 as usize] += 1;
        }
        out
    };
    let mut collisions = 0;
    for _ in 0..200 {
        c.step();
        free.step();
        collisions += c.collisions();
        assert_eq!(marginals(&c), marginals(&free), "a projection noticed a collision");
    }
    assert_ne!(positions(&c), positions(&free), "the gases themselves differ");
    println!("{collisions} collisions in 200 ticks, walls included, and all three 1-D density profiles equal a collisionless gas's");
    for permille in [50, 300, 700] {
        let (law, _) = cloud(Rule::LAW, permille);
        println!("law, density {:.2}: spread growth per axis at t = 32 relative to no collisions: {law:.6?}", permille as f64 / 1000.0);
        assert!(law.iter().all(|&v| (v - 1.0).abs() < 1e-9), "the law's gas never diffuses in projection");
    }
}

#[test]
fn the_literal_rule_diffuses_but_breaks_momentum() {
    for permille in [50, 300, 700] {
        let (s, moved) = cloud(Rule::LITERAL, permille);
        println!(
            "literal rule, density {:.2}: spread growth per axis (x, y, z) at t = 32 relative to no collisions: {s:.3?}; momentum moved by {moved}",
            permille as f64 / 1000.0
        );
        if permille == 700 {
            assert!(s.iter().all(|&v| v < 0.8), "dense gas diffuses");
            assert!(moved > 0, "the unchanged patterns reverse momentum");
        }
    }
}

#[test]
fn poke_emits_particles_and_the_ledger_counts_them() {
    let mut m = Matter::new(Grid::new(16, 16, 16));
    let wall = m.intern(&opaque());
    m.fill((0, 0, 0), (15, 15, 3), wall);
    let mut c = Margolus::new(m);
    c.poke(idx(&c, 8, 8, 8), 2);
    assert_eq!(c.particles(), 27);
    c.poke(idx(&c, 8, 8, 1), 1);
    assert_eq!(c.particles(), 27, "walls take no particle");
    c.poke(idx(&c, 8, 8, 4), 2);
    assert_eq!(c.ledger().get("emitted"), Some(27 + 18), "only the open cells of the second burst");
    let before = c.matter().cells().to_vec();
    for _ in 0..500 {
        c.step();
    }
    assert_eq!(c.particles(), 45);
    assert_eq!(c.matter().cells(), &before[..], "the law never changes matter");
    c.poke(idx(&c, 8, 8, 8), -8);
    println!("ledger after two bursts, 500 ticks and a sweep: {:?}", c.ledger());
    assert_eq!(c.ledger().get("particles"), Some(c.emitted()));
}

#[test]
fn the_lab_entry_builds_a_universe_at_rest() {
    let mut m = Matter::new(Grid::new(8, 8, 8));
    let wall: Mat = m.intern(&opaque());
    m.fill((0, 0, 0), (7, 7, 3), wall);
    let mut c = (ENTRY.build)(m);
    assert_eq!(c.id(), ENTRY.id);
    let d = c.digest();
    c.step();
    assert_eq!(c.active(), 0);
    c.step();
    assert_eq!(c.digest(), d, "nothing moves at rest");
}

#[test]
fn one_extra_particle_spreads_through_the_gas() {
    // Sinai's point: a gas of hard balls amplifies any difference. Two boxes differ by one particle;
    // count the cells whose bits differ as the difference spreads by collisions.
    let mut a = Margolus::new(Matter::new(Grid::new(32, 32, 32)));
    sprinkle(&mut a, (0, 0, 0), (31, 31, 31), 300, 21);
    let mut b = a.clone();
    let spot = (0..a.matter().grid.len()).find(|&i| !a.has(i)).expect("an empty cell");
    b.put(spot);
    let mut line = Vec::new();
    for t in 1..=256u32 {
        a.step();
        b.step();
        if t.is_power_of_two() {
            let differ = (0..a.matter().grid.len()).filter(|&i| a.has(i) != b.has(i)).count();
            line.push((t, differ));
        }
    }
    println!("cells that differ after one extra particle (tick, cells): {line:?}");
    assert!(line.last().expect("measured").1 > 1000, "the difference spreads through the gas");
    for _ in 0..256 {
        a.unstep();
        b.unstep();
    }
    let differ = (0..a.matter().grid.len()).filter(|&i| a.has(i) != b.has(i)).count();
    assert_eq!(differ, 1, "and unsteps back to the single particle");
}
