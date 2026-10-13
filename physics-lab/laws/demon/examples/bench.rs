//! Release-mode benchmark of the demon law on a 64³ box about half filled with a terrain-like slab
//! of real materials (a sink rock, an oxidizer layer and a fuel seam), at rest and while burning.
//!
//! `cargo run --release -p pwc_law_demon --example bench`

use std::time::Instant;

use pwc_law_core::search::{bond, find, is_cohesive};
use pwc_law_core::{Candidate, Grid, Matter};
use pwc_law_demon::Demon;
use pwc_mod_api::material::{cohesion, Block};

/// The best move's binding changes and needs, and the contact's ignition heat.
fn contact(a: &Block, b: &Block) -> (i32, u32, u32) {
    let mut m = Matter::new(Grid::new(2, 1, 1));
    let (ia, ib) = (m.intern(a), m.intern(b));
    let mut d = Demon::new(m);
    let ms = d.moves(ia, ib);
    let Some(best) = ms.first() else { return (0, u32::MAX, u32::MAX) };
    let ign = ms.iter().map(|v| v.need[0].max(v.need[1])).min().unwrap();
    (best.delta[0].max(best.delta[1]), best.need[0].max(best.need[1]), ign)
}

/// The experiments' materials (same searches as `src/tests.rs`).
fn materials() -> (Block, Block, Block) {
    let f = find(2, 3, 8, 100_000, |b| is_cohesive(b) && cohesion(b) > 40 && bond(b, b) > 20);
    let x = (100..1100)
        .map(|seed| find(seed, 3, 8, 100_000, |b| is_cohesive(b) && bond(b, b) > 20))
        .find(|x| {
            let (rel, need, _) = contact(&f, x);
            is_cohesive(x) && bond(x, x) > 300 && bond(&f, x) > 100 && rel > need as i32 && rel <= 2 * need as i32
        })
        .expect("an oxidizer");
    let s = find(7002, 3, 19, 1_000_000, |s| is_cohesive(s) && bond(s, s) > 150 && bond(s, &x) > 20 && contact(s, s).2 > 3000);
    (f, x, s)
}

fn run(d: &mut Demon, ticks: u32) -> (f64, u64) {
    let cells = d.matter().grid.len() as f64;
    let mut active = 0u64;
    let t0 = Instant::now();
    for _ in 0..ticks {
        d.step();
        active += d.active() as u64;
    }
    let ns = t0.elapsed().as_nanos() as f64 / (cells * f64::from(ticks));
    (ns, active)
}

fn main() {
    let (f, x, s) = materials();
    let g = Grid::new(64, 64, 64);
    let mut m = Matter::new(g);
    let (mf, mx, ms) = (m.intern(&f), m.intern(&x), m.intern(&s));
    m.fill((0, 0, 0), (63, 23, 63), ms);
    m.fill((0, 24, 0), (63, 28, 63), mx);
    m.fill((0, 29, 0), (63, 31, 63), mf);
    let filled = m.cells().iter().filter(|&&c| c != 0).count();
    let mut d = Demon::new(m);
    let need = contact(&f, &x).2;
    println!("demon bench: 64³ = {} cells, {filled} filled ({:.0}%), palette {}", g.len(), 100.0 * filled as f64 / g.len() as f64, d.matter().palette_len());
    println!("state: {} bytes per cell (heat u32); scratch: 1 flag byte + 8-byte conduction sum per cell", d.state_bytes_per_cell());

    let (ns, _) = run(&mut d, 1000);
    println!("at rest:  {ns:.4} ns per cell-tick ({:.3e} cells/s), active {}", 1.0e9 / ns, d.active());

    // Sparks: 16 fires on the fuel seam, three times its ignition heat. In 3-D they fizzle.
    for k in 0..16u32 {
        let (cx, cz) = (4 + (k % 4) * 16, 4 + (k / 4) * 16);
        d.poke(g.idx(cx, 31, cz), 3 * need as i32);
        d.poke(g.idx(cx, 30, cz), 3 * need as i32);
    }
    let e0 = d.energy().2;
    phase(&mut d, "sparks", 3);
    // The whole seam (every fuel and oxidizer cell) heated to its ignition temperature: it all goes.
    for z in 0..64 {
        for y in 24..32 {
            for x in 0..64 {
                d.poke(g.idx(x, y, z), need as i32);
            }
        }
    }
    phase(&mut d, "seam at ignition", 10);
    assert_eq!(d.energy().2, e0, "the ledger closes");
    println!("ledger: {:?}", d.ledger().entries);
}

fn phase(d: &mut Demon, label: &str, rounds: u32) {
    let cells = d.matter().grid.len() as f64;
    for _ in 0..rounds {
        let ticks = if rounds > 3 { 20u32 } else { 100 };
        let p0 = d.matter().palette_len();
        let r0 = d.reactions();
        let (ns, active) = run(d, ticks);
        let per_tick = active as f64 / f64::from(ticks);
        println!(
            "{label}: {ns:.3} ns per cell-tick ({:.3e} cells/s), {per_tick:.0} active cells/tick, {:.0} ns per active cell, {} moves, palette {p0} -> {}",
            1.0e9 / ns,
            ns * cells / per_tick.max(1.0),
            d.reactions() - r0,
            d.matter().palette_len()
        );
    }
}
