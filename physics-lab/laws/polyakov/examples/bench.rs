//! Release benchmark of the `polyakov` law on a 64³ box about half filled with terrain: insulating
//! rock with a conductor vein and dielectric crystals, a lossy soil on top, air above. Reports ns per
//! cell-tick and cells per second at rest, while busy after pokes, and with every row forced live,
//! plus what happens to the field energy over a long run.

use std::time::Instant;

use pwc_law_core::search::{find, is_cohesive};
use pwc_law_core::{Candidate, Grid, Matter};
use pwc_law_polyakov::Polyakov;
use pwc_mod_api::material::{probe_response, Block, Law};

const N: u32 = 64;

fn light(b: &Block) -> i32 {
    probe_response(b, Law::current().probes.light)
}

fn glow(b: &Block) -> i32 {
    probe_response(b, Law::current().probes.glow)
}

fn terrain() -> Matter {
    let rock = find(10, 3, 16, 200_000, |b| is_cohesive(b) && glow(b) <= 0 && (0..=200).contains(&light(b)));
    let soil = find(4, 3, 16, 200_000, |b| is_cohesive(b) && (48..=96).contains(&glow(b)) && light(b) <= 0);
    let ore = find(1, 3, 16, 200_000, |b| is_cohesive(b) && glow(b) >= 256 && light(b) <= 0);
    let crystal = find(2, 3, 16, 200_000, |b| is_cohesive(b) && glow(b) <= 0 && light(b) >= 512);
    let mut m = Matter::new(Grid::new(N, N, N));
    let (r, s, o, c) = (m.intern(&rock), m.intern(&soil), m.intern(&ore), m.intern(&crystal));
    let n = N as i32;
    m.fill((0, 0, 0), (n - 1, 25, n - 1), r);
    m.fill((0, 26, 0), (n - 1, 31, n - 1), s);
    m.fill((0, 14, 20), (n - 1, 16, 22), o);
    m.fill((20, 18, 0), (22, 20, n - 1), o);
    for k in 0..6 {
        let x = 6 + 10 * k;
        m.fill((x, 4, x / 2), (x + 3, 8, x / 2 + 5), c);
    }
    m
}

/// Run `ticks` steps; returns (ns per cell-tick, mean active fraction).
fn run(c: &mut Polyakov, ticks: usize, force: bool) -> (f64, f64) {
    let cells = c.matter().grid.len() as f64;
    let mut active = 0.0;
    let t0 = Instant::now();
    for _ in 0..ticks {
        if force {
            c.wake_all();
        }
        c.step();
        active += c.active() as f64;
    }
    let ns = t0.elapsed().as_nanos() as f64 / (ticks as f64 * cells);
    (ns, active / (ticks as f64 * cells))
}

fn report(name: &str, (ns, active): (f64, f64)) {
    println!("{name:<34} {ns:>7.2} ns/cell-tick {:>8.1} M cells/s   active {:>5.1} %", 1e3 / ns, 100.0 * active);
}

fn main() {
    let m = terrain();
    let filled = m.cells().iter().filter(|&&x| x != 0).count();
    let mut c = Polyakov::new(m);
    let cells = c.matter().grid.len();
    println!("polyakov bench: {N}³ = {cells} cells, {:.0} % matter, palette {}", 100.0 * filled as f64 / cells as f64, c.matter().palette_len());
    let wires = c.conducting_faces();
    println!("state {} bytes/cell (D 6, B 6, rho 2); cache 3 bytes/cell (eps per face) + {wires} conducting faces x {} bytes \
              ({:.1} bytes/cell); scratch 6 bytes/cell (E, then H); 1 byte per row",
        c.state_bytes_per_cell(), 12, 12.0 * wires as f64 / cells as f64);
    report("at rest", run(&mut c, 2000, false));

    let g = c.matter().grid;
    for k in 0..64u32 {
        let (x, z) = (3 + (k * 37) % 58, 3 + (k * 53) % 58);
        c.poke(g.idx(x, 32 + k % 8, z), 4000);
    }
    report("busy (64 pokes, first 200 ticks)", run(&mut c, 200, false));
    report("busy (ticks 200..1200)", run(&mut c, 1000, false));
    report("every row forced live", run(&mut c, 500, true));

    // Where does the energy go? Soil and ore are lossy; air and rock are not. Rounding heats every
    // live cell a little each tick; the lossy ground has to carry that away (about 3 minutes).
    println!("long run: tick, field energy, Joule so far, active %");
    let mut t = 1700;
    for chunk in [300, 1000, 3000, 5000, 10000, 20000, 20000, 20000, 20000, 20000] {
        let (_, active) = run(&mut c, chunk, false);
        t += chunk;
        let l = c.ledger();
        let rms = (l.get("energy").unwrap_or(0) as f64 * 2.0 / (6.0 * cells as f64)).sqrt();
        println!("  {t:>6} {:>14} {:>16} {:>6.1}   rms field {rms:.0}", l.get("energy").unwrap_or(0), c.joule(), 100.0 * active);
    }
    let l = c.ledger();
    println!("ledger at the end: {:?}", l.entries);
}
