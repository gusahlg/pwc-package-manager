//! The experiments. Every material is found by a deterministic search over the game's elements
//! (never hard-coded bytes), every circuit is drawn as a one-cell-thick sheet, and every test
//! asserts the measured phenomenon. Run with `--nocapture` to see the pictures.
//!
//! Every inequality a circuit relies on holds by at least [`MARGIN`] (1/8 `QUANTUM`), so no
//! result rests on a knife edge of the readings.

use std::sync::OnceLock;

use crate::material::{observe, Block, Law};
use crate::{drive, period, Belousov, GLOW_FROM, RECOVERY};
use pwc_law_core::search::{bond, find, is_cohesive, pure, Stream};
use pwc_law_core::{show, Candidate, Grid, Matter, Reading, VOID};

/// Slack every searched inequality keeps (Q8 of `QUANTUM`).
const MARGIN: i32 = 32;

fn reading(b: &Block) -> Reading {
    let mut m = Matter::new(Grid::new(1, 1, 1));
    let i = m.intern(b);
    *m.reading(i)
}

fn dr(b: &Block) -> i32 {
    drive(&reading(b))
}

fn per(b: &Block) -> u8 {
    period(&reading(b))
}

/// The period is well inside its band: the cohesion lies at least [`MARGIN`] from a step of
/// `RECOVERY`, or clearly below zero.
fn settled(b: &Block) -> bool {
    let c = reading(b).cohesion;
    c <= -MARGIN || (c % RECOVERY >= MARGIN && c % RECOVERY <= RECOVERY - MARGIN)
}

/// `a` fires when one neighbour of `b` is excited.
fn excites(a: &Block, b: &Block) -> bool {
    bond(a, b) + dr(a) > MARGIN
}

/// `a` stays at rest when one neighbour of `b` is excited.
fn holds(a: &Block, b: &Block) -> bool {
    bond(a, b) + dr(a) <= -MARGIN
}

/// A family of materials found by search, each defined only by its bonds and readings.
struct Kit {
    /// Wire: excites itself, never fires alone.
    w: Block,
    /// Junction: fires only when two wire neighbours are excited, and then fires a wire.
    j: Block,
    /// Second wire that inhibits the first where they touch, and is fired by the junction.
    v: Block,
    /// Pacemaker that drives the wire, with the wire's period.
    g: Block,
    /// Pacemaker faster than the wire can follow (period 3).
    fast: Block,
    /// Pacemaker one tick slower than the wire.
    slow: Block,
    /// Wire one tick slower to recover than `w`, joined to it both ways.
    lag: Block,
    /// Ordinary cohesive matter that never conducts and never fires alone.
    rock: Block,
}

fn kit() -> &'static Kit {
    static KIT: OnceLock<Kit> = OnceLock::new();
    KIT.get_or_init(|| {
        let n = 2_000_000;
        let w = find(101, 2, 16, n, |b| dr(b) <= -MARGIN && excites(b, b) && settled(b));
        let j = find(102, 2, 16, n, |b| {
            dr(b) <= -MARGIN && holds(b, &w) && 2 * bond(b, &w) + dr(b) > MARGIN && excites(&w, b)
        });
        let v = find(103, 2, 16, n, |b| {
            dr(b) <= -MARGIN
                && excites(b, b)
                && excites(b, &j)
                && holds(b, &w)
                && bond(&w, &w) + bond(&w, b) + dr(&w) <= -MARGIN
        });
        let pw = per(&w);
        let g = find(104, 2, 16, n, |b| dr(b) > MARGIN && excites(&w, b) && per(b) == pw && settled(b));
        let fast = find(105, 2, 16, n, |b| dr(b) > MARGIN && excites(&w, b) && per(b) == 3 && settled(b));
        let slow = find(106, 2, 16, n, |b| dr(b) > MARGIN && excites(&w, b) && per(b) == pw + 1 && settled(b));
        let rock = find(107, 3, 16, n, |b| {
            is_cohesive(b) && reading(b).cohesion > 0 && dr(b) <= -MARGIN && holds(b, b) && holds(b, &w)
        });
        let lag = find(108, 2, 16, n, |b| {
            dr(b) <= -MARGIN && excites(b, b) && per(b) == pw + 1 && settled(b) && excites(b, &w) && excites(&w, b)
        });
        assert!(pw > 3, "the divider experiment needs a wire slower than period 3");
        Kit { w, j, v, g, fast, slow, lag, rock }
    })
}

/// A one-cell-thick sheet drawn as text: row 0 is the top (highest y), `.` is void and every other
/// character is looked up in `legend`. Returns the law at rest and the picture's labels.
fn sheet(rows: &[&str], legend: &[(char, &Block)]) -> (Belousov, Labels) {
    let (nx, ny) = (rows.iter().map(|r| r.len()).max().unwrap() as u32, rows.len() as u32);
    let grid = Grid::new(nx, ny, 1);
    let mut m = Matter::new(grid);
    let mut at = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '.' {
                continue;
            }
            let b = legend.iter().find(|(c, _)| *c == ch).unwrap_or_else(|| panic!("no legend for {ch}")).1;
            let i = grid.idx(x as u32, ny - 1 - r as u32, 0);
            let mat = m.intern(b);
            m.set(i, mat);
            at.push((ch, i));
        }
    }
    (Belousov::new(m), Labels(at))
}

/// Where each character of a picture landed.
struct Labels(Vec<(char, usize)>);

impl Labels {
    fn all(&self, c: char) -> Vec<usize> {
        self.0.iter().filter(|(ch, _)| *ch == c).map(|&(_, i)| i).collect()
    }

    fn one(&self, c: char) -> usize {
        let v = self.all(c);
        assert_eq!(v.len(), 1, "label {c} must be unique");
        v[0]
    }
}

/// Run `ticks` ticks and return, per watched cell, the ticks after which it was excited.
fn record(law: &mut Belousov, ticks: i64, watch: &[usize]) -> Vec<Vec<i64>> {
    let mut out = vec![Vec::new(); watch.len()];
    for t in 1..=ticks {
        law.step();
        for (k, &i) in watch.iter().enumerate() {
            if law.excited(i) {
                out[k].push(t);
            }
        }
    }
    out
}

fn wire(len: u32, b: &Block) -> Belousov {
    let mut m = Matter::new(Grid::new(len, 1, 1));
    let mat = m.intern(b);
    m.fill((0, 0, 0), (len as i32 - 1, 0, 0), mat);
    Belousov::new(m)
}

#[test]
fn pacemakers_are_exactly_the_matter_that_shines() {
    let law = Law::current();
    let mut s = Stream::new(1);
    let (mut pace, mut cond, mut total) = (0, 0, 0);
    for _ in 0..20_000 {
        let kinds = 1 + s.below(4) as usize;
        let parts: Vec<_> = (0..kinds).map(|_| (s.element(), 16 / kinds)).collect();
        let b = pwc_law_core::search::mix(&parts);
        let r = reading(&b);
        let pacemaker = drive(&r) > 0;
        assert_eq!(pacemaker, observe(&law, &b).emission > 0, "a clock is exactly a block that shines");
        if pacemaker {
            pace += 1;
        } else if bond(&b, &b) + drive(&r) > 0 {
            cond += 1;
        }
        if kinds == 1 {
            assert!(pacemaker || bond(&b, &b) + drive(&r) <= 0, "a pure block never conducts");
        }
        total += 1;
    }
    let k = kit();
    println!(
        "census of {total} random configurations: {:.1}% pacemakers, {:.1}% self-conducting, {:.1}% inert",
        100.0 * pace as f64 / total as f64,
        100.0 * cond as f64 / total as f64,
        100.0 * (total - pace - cond) as f64 / total as f64
    );
    for (name, b) in [("w", &k.w), ("j", &k.j), ("v", &k.v), ("g", &k.g), ("fast", &k.fast), ("slow", &k.slow), ("lag", &k.lag), ("rock", &k.rock)] {
        let r = reading(b);
        println!(
            "{name:>4}: glow {:>5} drive {:>5} cohesion {:>4} period {} self-bond {:>5} bond to w {:>5}",
            r.glow,
            drive(&r),
            r.cohesion,
            period(&r),
            bond(b, b),
            bond(b, &k.w)
        );
    }
    println!("firing line GLOW_FROM = {GLOW_FROM}");
}

#[test]
fn a_pulse_runs_at_one_cell_per_tick_in_every_wire() {
    let k = kit();
    for (name, b) in [("w", &k.w), ("v", &k.v)] {
        let len = 400;
        let mut law = wire(len, b);
        law.poke(0, 1);
        let watch: Vec<usize> = (0..len as usize).collect();
        let fired = record(&mut law, len as i64 + 20, &watch);
        for (x, f) in fired.iter().enumerate().skip(1) {
            assert_eq!(f, &vec![x as i64], "wire {name}: cell {x} fires once, at tick {x}");
        }
        println!("wire {name}: a pulse crossed {len} cells in {} ticks: speed 1 cell/tick, undiminished", len - 1);
    }
    // The refractory period caps the pulse rate: shocking the end every tick sends one pulse per P.
    let mut law = wire(40, &k.w);
    let p = i64::from(per(&k.w));
    let mut far = Vec::new();
    for t in 1..=200 {
        law.poke(0, 1);
        law.step();
        if law.excited(30) {
            far.push(t);
        }
    }
    let gaps: Vec<i64> = far.windows(2).map(|w| w[1] - w[0]).collect();
    println!("shocked every tick, the far end fires every {:?} ticks (period {p})", &gaps[..4]);
    assert!(gaps.iter().all(|&g| g == p));
}

#[test]
fn refractoriness_makes_pulses_one_way_and_collisions_annihilate() {
    let k = kit();
    let len = 41;
    let watch: Vec<usize> = (0..len).collect();
    // A shock in the middle sends two pulses outward; none comes back.
    let mut law = wire(len as u32, &k.w);
    law.poke(20, 1);
    println!("{}", show::spacetime(&mut law, 0, 0, 24));
    let mut law = wire(len as u32, &k.w);
    law.poke(20, 1);
    let fired = record(&mut law, 200, &watch);
    for (x, f) in fired.iter().enumerate() {
        if x != 20 {
            assert_eq!(f, &vec![(x as i64 - 20).abs()], "cell {x} fires once");
        }
    }
    // Two pulses meeting head-on annihilate: every cell fires once and the wire falls silent.
    let mut law = wire(len as u32, &k.w);
    law.poke(0, 1);
    law.poke(len - 1, 1);
    let fired = record(&mut law, 200, &watch);
    assert!(fired.iter().enumerate().all(|(x, f)| x == 0 || x == len - 1 || f.len() == 1));
    assert_eq!(fired[20], vec![20]);
    assert_eq!(law.ledger().get("excited"), Some(0));
    assert_eq!(law.active(), 0, "the wire is at rest again and costs nothing");
    println!("head-on collision at cell 20, tick 20; wire at rest after, {} firings in all", law.fired());
}

#[test]
fn pure_and_ordinary_matter_stays_dark() {
    // A negative result: identical elements fit 0, so no pure block conducts; neither does
    // ordinary cohesive matter. Shocking them excites only the shocked cell.
    let k = kit();
    let mut s = Stream::new(9);
    let mut tried = 0;
    while tried < 200 {
        let b = pure(s.element(), 16);
        if dr(&b) > 0 {
            continue;
        }
        tried += 1;
        let mut law = wire(10, &b);
        law.poke(0, 1);
        let fired = record(&mut law, 20, &[1]);
        assert!(fired[0].is_empty(), "a pure block never passes a pulse");
    }
    let mut law = wire(10, &k.rock);
    law.poke(0, 1);
    let fired = record(&mut law, 20, &[1]);
    assert!(fired[0].is_empty());
    assert_eq!(law.fired(), 0);
    println!("200 pure non-shining blocks and the rock: no pulse passes");
}

#[test]
fn a_two_by_two_junction_is_a_diode() {
    // The pulse enters at a corner, splits into two cells that reach the junction together, and
    // the junction fires only on two inputs. From the other side it gets one, so it stays dark.
    let k = kit();
    let pic = ["iwwwwww.....", ".....wdwwwwo"];
    let fwd = [('i', &k.w), ('w', &k.w), ('d', &k.j), ('o', &k.w)];
    let (mut law, at) = sheet(&pic, &fwd);
    law.poke(at.one('i'), 1);
    let f = record(&mut law, 40, &[at.one('o'), at.one('d')]);
    println!("forward: junction fires at {:?}, output at {:?}", f[1], f[0]);
    assert_eq!(f[0], vec![12]);
    let (mut law, at) = sheet(&pic, &fwd);
    law.poke(at.one('o'), 1);
    let f = record(&mut law, 40, &[at.one('i'), at.one('d')]);
    println!("backward: junction fires {:?}, input end {:?}", f[1], f[0]);
    assert!(f[0].is_empty() && f[1].is_empty(), "the diode blocks the reverse pulse");
    // Control: with wire in place of the junction, the same shape conducts both ways.
    let ctl = [('i', &k.w), ('w', &k.w), ('d', &k.w), ('o', &k.w)];
    let (mut law, at) = sheet(&pic, &ctl);
    law.poke(at.one('o'), 1);
    let f = record(&mut law, 40, &[at.one('i')]);
    assert!(!f[0].is_empty());
}

#[test]
fn a_pacemaker_ticks_with_the_period_its_cohesion_sets() {
    let k = kit();
    let pw = i64::from(per(&k.w));
    for (name, g, want) in [("g", &k.g, pw), ("slow", &k.slow, pw + 1), ("fast", &k.fast, 3 * ((pw + 2) / 3))] {
        let (mut law, at) = sheet(&["cwwwwwwwwwwwo"], &[('c', g), ('w', &k.w), ('o', &k.w)]);
        let f = record(&mut law, 300, &[at.one('c'), at.one('o')]);
        let gaps = |v: &Vec<i64>| v.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
        let (gc, go) = (gaps(&f[0]), gaps(&f[1]));
        println!(
            "pacemaker {name}: period {} (cohesion {}), beats every {:?}; the wire (period {pw}) carries one every {:?}",
            per(g),
            reading(g).cohesion,
            &gc[..3],
            &go[..3]
        );
        assert!(gc.iter().all(|&x| x == i64::from(per(g))), "the clock's period is its own P");
        assert!(go.iter().all(|&x| x == want));
    }
    println!("a clock faster than the wire is divided: the wire takes the first beat after it recovers");
}

/// The two-input picture: inputs `a` and `b` reach the junction `x` at the same tick.
const GATE: [&str; 5] = ["awwww.....", "....w.....", "....xwwwwo", "....w.....", "bwwww....."];

#[test]
fn and_or_not_from_bonds_alone() {
    let k = kit();
    let mut table = String::new();
    for (name, x) in [("AND", &k.j), ("OR", &k.w)] {
        for (a, b) in [(false, false), (true, false), (false, true), (true, true)] {
            let (mut law, at) = sheet(&GATE, &[('a', &k.w), ('b', &k.w), ('w', &k.w), ('x', x), ('o', &k.w)]);
            if a {
                law.poke(at.one('a'), 1);
            }
            if b {
                law.poke(at.one('b'), 1);
            }
            let f = record(&mut law, 40, &[at.one('o')]);
            let out = !f[0].is_empty();
            table.push_str(&format!("{name} {} {} -> {}\n", a as u8, b as u8, out as u8));
            assert_eq!(out, if name == "AND" { a && b } else { a || b });
        }
    }
    // NOT: a clocked wire passes each beat unless the second wire, which inhibits it, fires
    // beside the gate cell at the very tick the beat arrives.
    let pic = ["cwwwwwnwwwwo", "......v.....", "......v.....", "......v.....", "......i....."];
    let legend = [('c', &k.g), ('w', &k.w), ('n', &k.w), ('o', &k.w), ('v', &k.v), ('i', &k.v)];
    let (mut law, at) = sheet(&pic, &legend);
    let before = at.one('n') - 1;
    let base = record(&mut law, 60, &[before, at.one('o')]);
    let beat = base[0][3];
    let run = |shift: i64| {
        let (mut law, at) = sheet(&pic, &legend);
        // Shocked before tick t, the input's end beside the gate fires at t + 2.
        let mut outs = Vec::new();
        for t in 1..=60 {
            if t == beat - 2 + shift {
                law.poke(at.one('i'), 1);
            }
            law.step();
            if law.excited(at.one('o')) {
                outs.push(t);
            }
        }
        outs
    };
    let inverted = run(0);
    let missing: Vec<i64> = base[1].iter().filter(|t| !inverted.contains(t)).copied().collect();
    table.push_str(&format!("NOT 0 -> 1 (beats out {:?})\nNOT 1 -> 0 (beat at {:?} suppressed)\n", &base[1][..5], missing));
    print!("{table}");
    assert_eq!(missing.len(), 1, "exactly the beat that met the input is suppressed");
    assert!(inverted.iter().all(|t| base[1].contains(t)));
    // The inhibition window is exactly one tick: an input one tick early or late does nothing.
    for shift in [-1, 1] {
        assert_eq!(run(shift), base[1], "an input off by {shift} tick does not invert");
    }
    println!("an input one tick early or late is ignored: inhibition must be exactly in phase");
}

#[test]
fn a_ring_remembers_its_pulses_forever() {
    let k = kit();
    let (w, h) = (12u32, 8u32);
    let mut m = Matter::new(Grid::new(w, h, 1));
    let mat = m.intern(&k.w);
    let mut ring = Vec::new();
    for x in 0..w {
        ring.push(m.grid.idx(x, 0, 0));
    }
    for y in 1..h {
        ring.push(m.grid.idx(w - 1, y, 0));
    }
    for x in (0..w - 1).rev() {
        ring.push(m.grid.idx(x, h - 1, 0));
    }
    for y in (1..h - 1).rev() {
        ring.push(m.grid.idx(0, y, 0));
    }
    for &i in &ring {
        m.set(i, mat);
    }
    let len = ring.len();
    let mut law = Belousov::new(m);
    // Write one bit: excite a cell and quench the one behind it, so the pulse runs one way.
    law.poke(ring[1], 1);
    law.poke(ring[0], -1);
    // Write a second bit half way round.
    law.poke(ring[len / 2 + 1], 1);
    law.poke(ring[len / 2], -1);
    for _ in 0..10_000 {
        law.step();
        assert_eq!(law.ledger().get("excited"), Some(2), "both pulses survive");
    }
    let d0 = law.digest();
    for _ in 0..len / 2 {
        law.step();
    }
    let half = law.digest();
    for _ in 0..len / 2 {
        law.step();
    }
    println!("{}", show::slice(&law, 0));
    assert_eq!(law.digest(), d0, "the state recurs exactly after one lap");
    assert_eq!(half, d0, "two evenly spaced pulses: the state recurs every half lap");
    println!(
        "ring of {len} cells holds 2 pulses for 10000 ticks, lap {len} ticks; capacity {} pulses (len / P)",
        len / per(&k.w) as usize
    );
}

/// A square sheet of wire with a wave along row `y0` from `x = 0` to `x1` (exclusive), quenched
/// behind. A full-width wave runs off the sheet; a broken one curls.
fn wave_sheet(n: u32, x1: u32) -> Belousov {
    let k = kit();
    let mut m = Matter::new(Grid::new(n, n, 1));
    let mat = m.intern(&k.w);
    m.fill((0, 0, 0), (n as i32 - 1, n as i32 - 1, 0), mat);
    let g = m.grid;
    let mut law = Belousov::new(m);
    let y0 = n / 2;
    for x in 0..x1 {
        law.poke(g.idx(x, y0, 0), 1);
        law.poke(g.idx(x, y0 - 1, 0), -1);
    }
    law
}

#[test]
fn a_broken_wave_curls_into_a_spiral_that_never_stops() {
    let n = 64;
    let mut plane = wave_sheet(n, n);
    let mut t_rest = 0;
    for t in 1..=200 {
        plane.step();
        if plane.live.is_empty() {
            t_rest = t;
            break;
        }
    }
    println!("a full-width wave leaves the sheet; at rest after {t_rest} ticks");
    assert!(t_rest > 0 && t_rest < 2 * n as i64);

    let mut law = wave_sheet(n, n / 2);
    for _ in 0..5_000 {
        law.step();
        assert!(law.ledger().get("excited").unwrap() > 0);
    }
    let pic = show::slice(&law, 0);
    let crop: Vec<String> = pic.lines().skip(20).take(24).map(|l| l.chars().skip(16).take(32).collect()).collect();
    println!("the centre of the 64 x 64 sheet after 5000 ticks:\n{}", crop.join("\n"));
    let d0 = law.digest();
    let mut period = 0;
    for t in 1..=400 {
        law.step();
        if law.digest() == d0 {
            period = t;
            break;
        }
    }
    let centre = law.matter.grid.idx(n / 2 + 10, n / 2, 0);
    let f = record(&mut law, 200, &[centre]);
    let gaps: Vec<i64> = f[0].windows(2).map(|w| w[1] - w[0]).collect();
    println!(
        "after 5000 ticks the spiral still turns: the whole sheet recurs exactly every {period} ticks; \
         a cell beats every {:?}; {} of {} cells active per tick",
        &gaps[..3],
        law.active(),
        n * n
    );
    assert!(period > 0, "the spiral is an exact limit cycle");
    assert!(gaps.iter().all(|&g| g == period));
    // A spiral is a phase defect: the phase winds once round any loop that encloses its tip,
    // and not at all round a loop that does not.
    let around = |cx: u32, cy: u32, r: u32| winding(&law, cx, cy, r);
    let (c, r) = (n / 2, [4, 8, 16, 24, 30]);
    let w: Vec<i32> = r.iter().map(|&r| around(c, c, r)).collect();
    let off = [around(10, 10, 5), around(50, 12, 6), around(12, 50, 6), around(50, 50, 6)];
    println!("phase winding round squares of radius {r:?} about the centre: {w:?}; round four squares away from it: {off:?}");
    assert!(w.iter().all(|&x| x.abs() == 1 && x == w[0]), "one spiral, one sense of rotation");
    assert!(off.iter().all(|&x| x == 0));
}

/// The phase winding (in turns) round the square of radius `r` about `(cx, cy)`: the sum of phase
/// steps between neighbours along the loop, each taken the short way round `Z/P`.
fn winding(law: &Belousov, cx: u32, cy: u32, r: u32) -> i32 {
    winding_z(law, cx, cy, r, 0)
}

/// [`winding`] in the z-slice `z`.
fn winding_z(law: &Belousov, cx: u32, cy: u32, r: u32, z: u32) -> i32 {
    let g = law.matter.grid;
    let (x0, y0, x1, y1) = (cx - r, cy - r, cx + r, cy + r);
    let mut ring = Vec::new();
    ring.extend((x0..x1).map(|x| g.idx(x, y0, z)));
    ring.extend((y0..y1).map(|y| g.idx(x1, y, z)));
    ring.extend((x0 + 1..=x1).rev().map(|x| g.idx(x, y1, z)));
    ring.extend((y0 + 1..=y1).rev().map(|y| g.idx(x0, y, z)));
    let p = i32::from(law.period_of(ring[0]));
    let mut sum = 0;
    for k in 0..ring.len() {
        let (a, b) = (i32::from(law.phase(ring[k])), i32::from(law.phase(ring[(k + 1) % ring.len()])));
        let d = (b - a).rem_euclid(p);
        let d = if 2 * d > p { d - p } else { d };
        assert!(2 * d.abs() < p, "neighbouring phases are never half a cycle apart");
        sum += d;
    }
    assert_eq!(sum % p, 0);
    sum / p
}

#[test]
fn a_half_adder_from_three_wire_materials() {
    // Each input forks to an OR cell (left) and an AND junction (right). The AND junction fires
    // the inhibiting wire `v`, whose pulse meets the OR's output at the gate cell `s` in the same
    // tick: S = A XOR B. The carry C is the `v` wire.
    let k = kit();
    let pic = [
        "..........a..........",
        "..........w..........",
        "..........w..........",
        "...wwwwwwwwwwwwwww...",
        "...w.............w...",
        "...w.............w...",
        "...wwwwwwwsvvcvvvj...",
        "...w.............w...",
        "...w.............w...",
        "...wwwwwwwwwwwwwww...",
        "..........w..........",
        "..........w..........",
        "..........b..........",
    ];
    let legend = [('a', &k.w), ('b', &k.w), ('w', &k.w), ('s', &k.w), ('v', &k.v), ('c', &k.v), ('j', &k.j)];
    for (a, b) in [(false, false), (true, false), (false, true), (true, true)] {
        let (mut law, at) = sheet(&pic, &legend);
        if a {
            law.poke(at.one('a'), 1);
        }
        if b {
            law.poke(at.one('b'), 1);
        }
        let mut f = vec![Vec::new(); 2];
        for t in 1..=80 {
            law.step();
            if t == 19 && a && b {
                println!("A=1 B=1 at tick 19: the inhibiting pulse meets the gate\n{}", show::slice(&law, 0));
            }
            for (q, &i) in [at.one('s'), at.one('c')].iter().enumerate() {
                if law.excited(i) {
                    f[q].push(t);
                }
            }
        }
        let (s, c) = (!f[0].is_empty(), !f[1].is_empty());
        println!("A={} B={} -> S={} C={}  (S at {:?}, C at {:?})", a as u8, b as u8, s as u8, c as u8, f[0], f[1]);
        assert_eq!((s, c), (a ^ b, a && b));
    }
}

#[test]
fn fan_out_regenerates_every_branch() {
    // Firing is all-or-none and every cell refires at full strength: one pulse feeds any number
    // of branches. That is gain, paid for by the drive, not by the pulse.
    let k = kit();
    // A comb: a spine along y with 16 teeth of length 20 along x.
    let teeth = 16;
    let mut m = Matter::new(Grid::new(22, 2 * teeth, 1));
    let mat = m.intern(&k.w);
    m.fill((0, 0, 0), (0, 2 * teeth as i32 - 1, 0), mat);
    for t in 0..teeth as i32 {
        m.fill((1, 2 * t, 0), (21, 2 * t, 0), mat);
    }
    let g = m.grid;
    let ends: Vec<usize> = (0..teeth).map(|t| g.idx(21, 2 * t, 0)).collect();
    let mut law = Belousov::new(m);
    law.poke(g.idx(0, 0, 0), 1);
    let f = record(&mut law, 200, &ends);
    for (t, e) in f.iter().enumerate() {
        assert_eq!(e, &vec![2 * t as i64 + 21], "tooth {t} fires once, on time");
    }
    assert_eq!(law.ledger().get("excited"), Some(0), "a tree without loops returns to rest");
    println!("one pulse reached all {teeth} branch ends at full strength; {} firings from 1 shock", law.fired());
}

/// The rule written densely and double buffered: the reference the sparse law must equal.
fn dense_step(m: &Matter, phase: &[u8]) -> Vec<u8> {
    let g = m.grid;
    (0..g.len())
        .map(|i| {
            let mat = m.get(i);
            let r = m.reading(mat);
            let f = phase[i];
            if mat == VOID {
                0
            } else if f != 0 {
                if f + 1 < period(r) { f + 1 } else { 0 }
            } else {
                let sum: i32 = drive(r)
                    + g.neighbours(i).iter().flatten().filter(|&&j| phase[j] == 1).map(|&j| m.bond(mat, m.get(j))).sum::<i32>();
                u8::from(sum > 0)
            }
        })
        .collect()
}

#[test]
fn the_sparse_law_equals_the_dense_rule() {
    let k = kit();
    let mut s = Stream::new(77);
    let mut m = Matter::new(Grid::new(12, 10, 8));
    let pal = [VOID, m.intern(&k.w), m.intern(&k.j), m.intern(&k.v), m.intern(&k.g), m.intern(&k.rock), m.intern(&k.fast)];
    let weights = [6, 10, 2, 4, 1, 3, 1];
    let total: u64 = weights.iter().sum();
    for i in 0..m.grid.len() {
        let mut r = s.below(total);
        let mut c = 0;
        while r >= weights[c] {
            r -= weights[c];
            c += 1;
        }
        m.set(i, pal[c]);
    }
    let mut law = Belousov::new(m.clone());
    let mut twin = Belousov::new(m.clone());
    let mut dense = vec![0u8; m.grid.len()];
    for _ in 0..500 {
        for _ in 0..3 {
            let i = s.below(m.grid.len() as u64) as usize;
            let st = if s.below(2) == 0 { 1 } else { -1 };
            law.poke(i, st);
            twin.poke(i, st);
            if m.get(i) != VOID {
                dense[i] = if st > 0 { 1 } else { 2 };
            }
        }
        law.step();
        twin.step();
        dense = dense_step(&m, &dense);
        assert_eq!(law.phase, dense, "the sparse law equals the dense rule");
        assert_eq!(law.digest(), twin.digest(), "two peers agree");
    }
    println!("500 ticks with 1500 pokes on 960 cells: sparse == dense every tick; final excited {:?}", law.ledger().get("excited"));
}

#[test]
fn nothing_is_conserved_a_pacemaker_fires_forever() {
    // The drive is free: a lone pacemaker in void fires every P ticks forever, and the count of
    // firings (what a fuel rule would have to pay for) grows without bound.
    let k = kit();
    let mut m = Matter::new(Grid::new(1, 1, 1));
    let g = m.intern(&k.g);
    m.set(0, g);
    let mut law = Belousov::new(m);
    let p = i64::from(per(&k.g));
    for _ in 0..100_000 {
        law.step();
    }
    assert_eq!(law.fired(), (100_000 + p - 1) / p);
    println!("one pacemaker, 100000 ticks: {} firings, nothing paid", law.fired());
}

#[test]
fn wave_trains_never_nucleate_spirals_on_their_own() {
    // A negative result. In real excitable tissue a wave train that meets a patch which recovers
    // more slowly breaks up into spirals (Krinsky's re-entry). Here it never does: in 64
    // geometries (8 patches, 4 train lengths, a slow conducting patch or an inert obstacle) the
    // waves wrap round, close behind and the sheet returns to rest. Spirals need a wave broken
    // by a quench (a player, or a timed inhibitory pulse).
    let k = kit();
    let n = 64u32;
    let patches = [(0, 30, 24, 34), (0, 20, 20, 40), (10, 30, 24, 30), (0, 63, 24, 30), (0, 31, 10, 12), (5, 40, 20, 21), (0, 40, 20, 20), (20, 22, 5, 50)];
    let mut runs = 0;
    for (x0, x1, y0, y1) in patches {
        for pulses in [2, 3, 5, 10] {
            for patch in [&k.lag, &k.rock] {
                let mut m = Matter::new(Grid::new(n, n, 1));
                let (w, p) = (m.intern(&k.w), m.intern(patch));
                m.fill((0, 0, 0), (n as i32 - 1, n as i32 - 1, 0), w);
                m.fill((x0, y0, 0), (x1, y1, 0), p);
                let g = m.grid;
                let mut law = Belousov::new(m);
                let pw = i64::from(per(&k.w));
                for t in 0..1000i64 {
                    if t % pw == 0 && t < pw * pulses {
                        for x in 0..n {
                            law.poke(g.idx(x, 0, 0), 1);
                        }
                    }
                    law.step();
                }
                assert_eq!(law.active(), 0, "patch {:?}, {pulses} pulses: the sheet is at rest", (x0, x1, y0, y1));
                runs += 1;
            }
        }
    }
    println!("{runs} wave trains past slow patches and obstacles: all returned to rest, no spiral nucleated");
}

#[test]
fn an_inhibitory_layer_breaks_a_wave_into_a_spiral() {
    // Two plain shocks, no quench: a line across a wire sheet and, one row ahead, a line across
    // a half sheet of the inhibiting wire underneath. The inhibiting wave leads the wire's wave
    // by one tick and kills it over the half sheet; the free end curls into a spiral.
    let k = kit();
    let n = 64u32;
    let mut m = Matter::new(Grid::new(n, n, 2));
    let (w, v) = (m.intern(&k.w), m.intern(&k.v));
    m.fill((0, 0, 1), (n as i32 - 1, n as i32 - 1, 1), w);
    m.fill((0, 0, 0), (n as i32 / 2 - 1, n as i32 - 1, 0), v);
    let g = m.grid;
    let mut law = Belousov::new(m);
    for x in 0..n {
        law.poke(g.idx(x, 20, 1), 1);
        law.poke(g.idx(x, 21, 0), 1);
    }
    for _ in 0..3_000 {
        law.step();
    }
    let excited = law.ledger().get("excited").unwrap();
    let wind = |cx, cy, r| winding_z(&law, cx, cy, r, 1);
    let w: Vec<i32> = [6, 12, 18].iter().map(|&r| wind(32, 24, r)).collect();
    println!("after 3000 ticks: {excited} cells excited; winding round the break point {w:?}");
    println!("{}", show::slice(&law, 1).lines().skip(24).take(20).collect::<Vec<_>>().join("\n"));
    assert!(excited > 0);
    assert!(w.iter().all(|&x| x.abs() == 1));
}
