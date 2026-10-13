use crate::grid::{opposite, Grid};
use crate::matter::{Matter, VOID};
use crate::search::{find, is_cohesive, pure, Stream};

#[test]
fn grid_indices_round_trip_and_steps_stay_inside() {
    let g = Grid::new(5, 4, 3);
    for i in 0..g.len() {
        let (x, y, z) = g.pos(i);
        assert_eq!(g.idx(x, y, z), i);
        for d in 0..6 {
            if let Some(j) = g.step(i, d) {
                assert_eq!(j as isize - i as isize, g.stride(d));
                assert_eq!(g.step(j, opposite(d)), Some(i));
            }
        }
    }
    assert_eq!(g.step(0, 0), None);
}

#[test]
fn interning_dedupes_and_bonds_are_symmetric() {
    let mut s = Stream::new(7);
    let mut m = Matter::new(Grid::new(2, 1, 1));
    let a = pure(s.element(), 8);
    let b = pure(s.element(), 8);
    let ia = m.intern(&a);
    let ib = m.intern(&b);
    assert_eq!(m.intern(&a), ia);
    assert_ne!(ia, ib);
    assert_eq!(m.bond(ia, ib), m.bond(ib, ia));
    assert_eq!(m.bond(VOID, ia), 0);
    // Identical elements fit 0, so a pure block bonds to itself with 0.
    assert_eq!(m.bond(ia, ia), 0);
    m.set(0, ia);
    m.set(1, ib);
    assert_eq!(m.occurrences(), 16);
}

#[test]
fn search_is_deterministic() {
    let a = find(11, 3, 12, 10_000, is_cohesive);
    let b = find(11, 3, 12, 10_000, is_cohesive);
    assert_eq!(a.configuration(), b.configuration());
}

#[test]
fn bonds_survive_the_table_growing() {
    let mut s = Stream::new(3);
    let mut m = Matter::new(Grid::new(1, 1, 1));
    let blocks: Vec<_> = (0..200).map(|_| pure(s.element(), 4)).collect();
    let ids: Vec<_> = blocks.iter().map(|b| m.intern(b)).collect();
    for (i, a) in blocks.iter().enumerate().step_by(7) {
        for (j, b) in blocks.iter().enumerate().step_by(11) {
            assert_eq!(m.bond(ids[i], ids[j]), crate::search::bond(a, b));
        }
    }
}
