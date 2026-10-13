//! Release benchmark of the `margolus` law: a 64³ box about half filled with a terrain-like slab of
//! real materials (two opaque rocks under a clear crust), at rest, after pokes, and as a dense gas.
//! Run: `cargo run --release -p pwc_law_margolus --example bench`.

use std::time::Instant;

use pwc_law_core::search::{find, find_with, is_cohesive};
use pwc_law_core::{Candidate, Grid, Matter};
use pwc_law_margolus::{Margolus, CLEAR};
use pwc_mod_api::material::{probe_response, Block, Law};

fn light(b: &Block) -> i32 {
    probe_response(b, Law::current().probes.light)
}

fn terrain() -> Matter {
    let n = 64;
    let mut m = Matter::new(Grid::new(n, n, n));
    let rock = m.intern(&find(1, 3, 12, 10_000, |b| is_cohesive(b) && light(b) <= CLEAR));
    let soil = m.intern(&find(5, 2, 10, 10_000, |b| is_cohesive(b) && light(b) <= CLEAR));
    let crust = m.intern(&find_with(2, 8, 100_000, |b| light(b) > CLEAR));
    for z in 0..n as i32 {
        for x in 0..n as i32 {
            // Gentle integer hills around y = 32.
            let h = 32 + ((x * 7 + z * 3) % 11 - 5) / 2 + ((x / 9 + z / 13) % 3) - 1;
            m.fill((x, 0, z), (x, h - 6, z), rock);
            m.fill((x, h - 5, z), (x, h - 2, z), soil);
            m.fill((x, h - 1, z), (x, h, z), crust);
        }
    }
    m
}

fn run(name: &str, c: &mut Margolus, ticks: u32) {
    let cells = c.matter().grid.len() as f64;
    for _ in 0..20 {
        c.step();
    }
    let t = Instant::now();
    let mut active = 0usize;
    for _ in 0..ticks {
        c.step();
        active += c.active();
    }
    let dt = t.elapsed().as_secs_f64();
    let per = dt * 1e9 / (cells * f64::from(ticks));
    let t = Instant::now();
    for _ in 0..ticks {
        c.unstep();
    }
    let back = t.elapsed().as_secs_f64() * 1e9 / (cells * f64::from(ticks));
    println!(
        "{name:<34} {:>8} particles  {per:>7.3} ns/cell-tick  {:>8.1} M cells/s  {:>6.1} µs/tick  unstep {back:.3} ns/cell-tick  active {:>5.1}%",
        c.particles(),
        1e-6 / (per * 1e-9),
        dt * 1e6 / f64::from(ticks),
        100.0 * active as f64 / (cells * f64::from(ticks))
    );
}

fn main() {
    let t = Instant::now();
    let m = terrain();
    let open = (0..m.grid.len()).filter(|&i| m.get(i) == 0).count();
    let mut c = Margolus::new(m);
    println!(
        "64³ box, {:.1}% matter; build incl. deriving the 64 KiB rule table: {:.1} ms",
        100.0 * (1.0 - open as f64 / c.matter().grid.len() as f64),
        t.elapsed().as_secs_f64() * 1e3
    );
    println!(
        "state: 1 bit per cell (state_bytes_per_cell rounds up to {}); derived wall/clear caches 8 bits per cell at nx = 64",
        c.state_bytes_per_cell()
    );
    run("at rest", &mut c, 2000);
    let g = c.matter().grid;
    for k in 0..8u32 {
        c.poke(g.idx(8 + 6 * k, 48, 8 + 6 * k), 4);
    }
    run("busy (8 pokes of radius 3)", &mut c, 2000);
    let mut dense = Margolus::new(c.matter().clone());
    let mut s = pwc_law_core::search::Stream::new(3);
    for i in 0..g.len() {
        if s.below(10) < 3 {
            dense.put(i);
        }
    }
    run("dense gas (30% of open cells)", &mut dense, 500);
    let mut full = Margolus::new(c.matter().clone());
    for i in 0..g.len() {
        full.put(i);
    }
    run("every open cell full", &mut full, 500);
}
