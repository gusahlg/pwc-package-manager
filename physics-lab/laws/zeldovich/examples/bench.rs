//! Release-mode benchmark of the zeldovich law on a 64³ box about half filled with a terrain-like
//! slab of real matter found by search: rock below, soil on top, and buried fuel pockets (3-D
//! checkerboards of a fuel pair).
//!
//! Cases: the slab at rest; busy with heat (64 pokes on the surface, transport only); busy with
//! fire (the eight buried pockets lit); and the worst case, an exposed fuel bed on top of the slab
//! lit at one corner (chemistry and interning included).

use std::time::Instant;

use pwc_law_core::search::{Stream, bond, find, is_cohesive, mix};
use pwc_law_core::{Candidate, Grid, Matter};
use pwc_law_zeldovich::{Zeldovich, min_barrier, react};
use pwc_mod_api::material::Block;

const N: u32 = 64;

/// A fuel pair: ignites between 10 and 40 quanta, releases at least twice that, does not conduct.
fn fuel(rock: &Block) -> (Block, Block) {
    let mut s = Stream::new(99);
    let one = |s: &mut Stream| {
        let k = 2 + s.below(2) as usize;
        let parts: Vec<_> = (0..k).map(|i| (s.element(), 8 / k + usize::from(i < 8 % k))).collect();
        mix(&parts)
    };
    loop {
        let (f, o) = (one(&mut s), one(&mut s));
        if !is_cohesive(&f) || !is_cohesive(&o) || bond(&f, &o) > 0 || bond(rock, &f) <= 0 || bond(&o, rock) <= 0 {
            continue;
        }
        let Some(th) = (1..=40).find(|&h| react(&f, &o, h).is_some()) else { continue };
        if th >= 10 && react(&f, &o, th).is_some_and(|m| m.release >= 2 * th) {
            return (f, o);
        }
    }
}

fn slab(bed: bool) -> (Zeldovich, Vec<usize>, Vec<usize>) {
    let rock = find(1, 3, 16, 100_000, |b| is_cohesive(b) && bond(b, b) > 0 && min_barrier(b) >= 150);
    let soil = find(2, 3, 16, 100_000, |b| {
        is_cohesive(b)
            && bond(b, b) > 0
            && min_barrier(b) >= 150
            && bond(b, &rock) > 0
            && b.configuration() != rock.configuration()
    });
    let (f, o) = fuel(&rock);
    let g = Grid::new(N, N, N);
    let mut m = Matter::new(g);
    let (ir, is, i_f, io) = (m.intern(&rock), m.intern(&soil), m.intern(&f), m.intern(&o));
    let top = N as i32 - 1;
    m.fill((0, 0, 0), (top, 27, top), ir);
    m.fill((0, 28, 0), (top, 31, top), is);
    if bed {
        // The top four layers are a fuel bed instead of soil.
        for z in 0..N as i32 {
            for y in 28..32 {
                for x in 0..N as i32 {
                    m.put(x, y, z, if (x + y + z) % 2 == 0 { i_f } else { io });
                }
            }
        }
    }
    // Eight fuel pockets of 6×3×6 at y = 10..13.
    let mut pockets = Vec::new();
    for k in 0..8 {
        let (x0, z0) = (4 + (k % 4) * 15, 8 + (k / 4) * 30);
        for z in z0..z0 + 6 {
            for y in 10..13 {
                for x in x0..x0 + 6 {
                    m.put(x, y, z, if (x + y + z) % 2 == 0 { i_f } else { io });
                }
            }
        }
        pockets.push(g.idx(x0 as u32, 10, z0 as u32));
    }
    let surface: Vec<usize> = (0..64u32).map(|k| g.idx(4 + (k % 8) * 8, 31, 4 + (k / 8) * 8)).collect();
    (Zeldovich::new(m), surface, pockets)
}

fn run(name: &str, z: &mut Zeldovich, ticks: usize) {
    let cells = z.matter().grid.len() as f64;
    let t0 = Instant::now();
    let mut active = 0usize;
    let mut moves = 0u64;
    for _ in 0..ticks {
        z.step();
        active += z.active();
        moves += z.reactions();
    }
    let dt = t0.elapsed().as_secs_f64();
    let per_tick = dt / ticks as f64;
    let mean_active = active as f64 / ticks as f64;
    println!(
        "{name:<28} {:>9.3} ns/cell-tick  {:>12.0} cells/s  {:>9.2} ms/tick  active {:>8.0} ({:>5.2}%)  {:>7.1} ns/active-cell-tick  moves/tick {:>8.1}  palette {}",
        per_tick * 1e9 / cells,
        cells / per_tick,
        per_tick * 1e3,
        mean_active,
        100.0 * mean_active / cells,
        if mean_active > 0.0 { per_tick * 1e9 / mean_active } else { 0.0 },
        moves as f64 / ticks as f64,
        z.matter().palette_len()
    );
}

fn main() {
    let only_heat = std::env::args().any(|a| a == "heat");
    let t0 = Instant::now();
    let (base, surface, pockets) = slab(false);
    println!("built 64³ slab in {:.2} s", t0.elapsed().as_secs_f64());
    let filled = base.matter().cells().iter().filter(|&&m| m != 0).count();
    println!(
        "cells {} ({} matter, {:.1}%), state {} bytes/cell (24 quanta counts + 1 rotor), scratch 29 bytes/cell (24 inbound + 1 open mask + 4 claim stamp + 2 activity bits)",
        N.pow(3),
        filled,
        100.0 * filled as f64 / f64::from(N.pow(3)),
        base.state_bytes_per_cell()
    );

    let mut z = Zeldovich::new(base.matter().clone());
    run("at rest", &mut z, 1000);

    let mut z = Zeldovich::new(base.matter().clone());
    for &c in &surface {
        z.poke(c, 600);
    }
    run("heat: first 100 ticks", &mut z, 100);
    run("heat: ticks 101..600", &mut z, 500);
    let l = z.ledger();
    println!("  heat ledger: {:?}", l.entries);
    if only_heat {
        return;
    }

    let mut z = Zeldovich::new(base.matter().clone());
    let e0 = z.ledger().get("energy");
    for &c in &pockets {
        z.poke(c, 2000);
    }
    run("fire: first 100 ticks", &mut z, 100);
    run("fire: ticks 101..400", &mut z, 300);
    let l = z.ledger();
    println!("  fire ledger: {:?}", l.entries);
    assert_eq!(l.get("energy"), e0, "energy drifted");

    let (mut z, _, _) = slab(true);
    let e0 = z.ledger().get("energy");
    z.poke(z.matter().grid.idx(0, 31, 0), 2000);
    run("inferno: first 25 ticks", &mut z, 25);
    run("inferno: ticks 26..50", &mut z, 25);
    run("inferno: ticks 51..75", &mut z, 25);
    run("inferno: ticks 76..100", &mut z, 25);
    let l = z.ledger();
    println!("  inferno ledger: {:?}", l.entries);
    assert_eq!(l.get("energy"), e0, "energy drifted");
}
