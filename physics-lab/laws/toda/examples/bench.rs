//! Release-mode benchmark of the `toda` law on a 64³ box about half filled with a terrain-like
//! slab of real matter: three self-bonding mixtures in strata and pockets of pure (unbonded)
//! matter. Reports ns per cell-tick and cells per second at rest, while busy after a few pokes,
//! and fully busy (every block kicked), forward and backward.

use std::time::Instant;

use pwc_law_core::search::{bond, find, pure, Stream};
use pwc_law_core::{Candidate, Grid, Matter, VOID};
use pwc_law_toda::Toda;

const N: u32 = 64;

fn world() -> Matter {
    let a = find(1, 2, 2, 1_000_000, |b| bond(b, b) >= 400);
    let b = find(2, 2, 8, 1_000_000, |b| bond(b, b) >= 350);
    let c = find(3, 2, 16, 1_000_000, |b| bond(b, b) >= 300);
    let mut s = Stream::new(9);
    let dead = pure(s.element(), 6);
    let mut m = Matter::new(Grid::new(N, N, N));
    let ids = [m.intern(&a), m.intern(&b), m.intern(&c), m.intern(&dead)];
    for z in 0..N as i32 {
        for x in 0..N as i32 {
            // Rolling hills around half height.
            let h = 30 + ((x as f64 / 7.0).sin() * 3.0 + (z as f64 / 9.0).cos() * 3.0) as i32;
            for y in 0..h {
                let depth = h - 1 - y;
                let mat = match depth {
                    0..=1 => ids[0],
                    2..=9 => ids[1],
                    _ => ids[2],
                };
                m.put(x, y, z, mat);
            }
        }
    }
    // Pockets of pure matter (deaf to itself) and a few caves.
    for _ in 0..40 {
        let (x, y, z) = (s.below(60) as i32, s.below(24) as i32, s.below(60) as i32);
        let fill = if s.below(2) == 0 { ids[3] } else { VOID };
        m.fill((x, y, z), (x + 3, y + 2, z + 3), fill);
    }
    m
}

fn run(name: &str, t: &mut Toda, ticks: usize, back: bool) {
    let cells = t.matter().grid.len() as f64;
    let mut worked = 0usize;
    let start = Instant::now();
    for _ in 0..ticks {
        if back {
            t.unstep();
        } else {
            t.step();
        }
        worked += t.active();
    }
    let ns = start.elapsed().as_nanos() as f64;
    let per_tick = ns / ticks as f64;
    let mean_active = worked as f64 / ticks as f64;
    println!(
        "{name:28} {:9.3} ns/cell-tick  {:8.2e} cells/s  {:7.0} active  {:6.2} ns/active-cell  {:9.1} µs/tick",
        per_tick / cells,
        cells / per_tick * 1e9,
        mean_active,
        if mean_active > 0.0 { per_tick / mean_active } else { 0.0 },
        per_tick / 1000.0
    );
}

fn main() {
    let m = world();
    let g = m.grid;
    let filled = m.cells().iter().filter(|&&c| c != VOID).count();
    let mut t = Toda::new(m);
    println!(
        "toda bench: {}³ box, {} cells, {filled} matter ({:.1}%), state {} bytes/cell (caches: +1 link mask, +4 era mark, +4 cached half impulse, worklists up to 12 at full activity)",
        N,
        g.len(),
        100.0 * filled as f64 / g.len() as f64,
        t.state_bytes_per_cell()
    );
    run("at rest", &mut t, 2000, false);
    let mut s = Stream::new(77);
    for _ in 0..16 {
        let (x, z) = (s.below(u64::from(N)) as u32, s.below(u64::from(N)) as u32);
        t.poke(g.idx(x, 20, z), 4_000_000);
    }
    let poked = t.digest();
    run("16 pokes, first 200 ticks", &mut t, 200, false);
    run("16 pokes, next 2000 ticks", &mut t, 2000, false);
    run("same, backward 2200 ticks", &mut t, 2200, true);
    println!("rewound to the pokes exactly: {}", t.digest() == poked);
    for i in 0..g.len() {
        t.poke(i, (s.next_u64() % 2_000_001) as i32 - 1_000_000);
    }
    let kicked = t.digest();
    run("every block kicked (busy)", &mut t, 1000, false);
    run("busy, backward", &mut t, 1000, true);
    println!("rewound to the kicks exactly: {}", t.digest() == kicked);
    let e = t.energy();
    println!("energy books: kinetic {:.3e} potential {:.3e} shadow {:.3e}", e.kinetic, e.potential, e.shadow);
}
