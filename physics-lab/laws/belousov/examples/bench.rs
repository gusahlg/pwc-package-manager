//! Release-mode benchmark of the excitable medium on a 64³ box about half filled with a
//! terrain-like slab of real matter: rock, a conducting layer and soil, found by search.
//!
//! Cases: the slab at rest; at rest with glowing "lamps" (pacemakers) in the soil; a scroll wave
//! turning in the conducting layer; and the worst case, a scroll wave filling the whole slab.

use std::time::Instant;

use pwc_law_belousov::{drive, period, Belousov};
use pwc_law_core::search::{bond, find, is_cohesive};
use pwc_law_core::{Candidate, Grid, Matter, Reading};
use pwc_mod_api::material::Block;

const N: u32 = 64;
const MARGIN: i32 = 32;

fn reading(b: &Block) -> Reading {
    let mut m = Matter::new(Grid::new(1, 1, 1));
    let i = m.intern(b);
    *m.reading(i)
}

fn dr(b: &Block) -> i32 {
    drive(&reading(b))
}

struct Slab {
    matter: Matter,
    /// Cells of the conducting layer, as `(lo, hi)` in y.
    layer: (u32, u32),
}

/// Rock below y = 20, the layer `y ∈ [20, 28)`, soil to y = 32; void above. `full` makes all of
/// it conducting; `lamps` puts a pacemaker in every 64th soil cell.
fn slab(full: bool, lamps: bool) -> Slab {
    let n = 2_000_000;
    let w = find(101, 2, 16, n, |b| dr(b) <= -MARGIN && bond(b, b) + dr(b) > MARGIN);
    let inert = |b: &Block| is_cohesive(b) && dr(b) <= -MARGIN && bond(b, b) + dr(b) <= -MARGIN && bond(b, &w) + dr(b) <= -MARGIN;
    let rock = find(107, 3, 16, n, inert);
    let soil = find(109, 3, 16, n, inert);
    let lamp = find(104, 2, 16, n, |b| dr(b) > MARGIN);
    let mut m = Matter::new(Grid::new(N, N, N));
    let (w, rock, soil, lamp) = (m.intern(&w), m.intern(&rock), m.intern(&soil), m.intern(&lamp));
    let top = N as i32 - 1;
    if full {
        m.fill((0, 0, 0), (top, 31, top), w);
    } else {
        m.fill((0, 0, 0), (top, 19, top), rock);
        m.fill((0, 20, 0), (top, 27, top), w);
        m.fill((0, 28, 0), (top, 31, top), soil);
    }
    if lamps {
        for z in 0..N {
            for x in 0..N {
                if (x * 7 + z * 13) % 64 == 0 {
                    m.put(x as i32, 30, z as i32, lamp);
                }
            }
        }
    }
    Slab { matter: m, layer: if full { (0, 32) } else { (20, 28) } }
}

/// A scroll wave: a plane at x = 32 across half the layer (z < 32), quenched behind.
fn scroll(law: &mut Belousov, layer: (u32, u32)) {
    let g = law.matter().grid;
    for z in 0..N / 2 {
        for y in layer.0..layer.1 {
            law.poke(g.idx(32, y, z), 1);
            law.poke(g.idx(31, y, z), -1);
        }
    }
}

fn run(name: &str, law: &mut Belousov, warm: usize, ticks: usize) {
    for _ in 0..warm {
        law.step();
    }
    let cells = law.matter().grid.len();
    let mut active = 0usize;
    let t = Instant::now();
    for _ in 0..ticks {
        law.step();
        active += law.active();
    }
    let secs = t.elapsed().as_secs_f64();
    let ns = secs * 1e9 / (cells * ticks) as f64;
    let per_active = if active > 0 { secs * 1e9 / active as f64 } else { 0.0 };
    println!(
        "{name:<24} {ns:>8.3} ns/cell-tick {:>10.3e} cells/s  {:>8.0} active/tick ({:>5.2}%)  {per_active:>6.2} ns/active cell  {:>7.1} us/tick",
        (cells * ticks) as f64 / secs,
        active as f64 / ticks as f64,
        100.0 * active as f64 / (cells * ticks) as f64,
        secs * 1e6 / ticks as f64
    );
}

fn main() {
    let filled = {
        let s = slab(false, false);
        s.matter.cells().iter().filter(|&&m| m != 0).count()
    };
    println!("box {N}³ = {} cells, {} filled ({:.0}%)", N * N * N, filled, 100.0 * filled as f64 / (N * N * N) as f64);
    let s = slab(false, false);
    let w = s.matter.reading(s.matter.get(s.matter.grid.idx(0, 20, 0)));
    println!("conducting layer: period {} ticks, drive {}", period(w), drive(w));

    let mut law = Belousov::new(slab(false, false).matter);
    println!("state: {} byte per cell (phase); scratch: O(active) lists", law.state_bytes_per_cell());
    run("rest", &mut law, 10, 10_000);

    let mut law = Belousov::new(slab(false, true).matter);
    run("rest + lamps", &mut law, 10, 2_000);

    let s = slab(false, false);
    let mut law = Belousov::new(s.matter);
    scroll(&mut law, s.layer);
    run("scroll wave in layer", &mut law, 500, 1_000);

    let s = slab(true, false);
    let mut law = Belousov::new(s.matter);
    scroll(&mut law, s.layer);
    run("scroll wave, full slab", &mut law, 500, 500);
    println!("fired in the full slab so far: {}", law.fired());
}
