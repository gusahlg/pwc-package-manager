# Changelog — pwc.law-margolus

## 0.1.0

- First release. The reversible particle universe: one particle bit per open cell, a Margolus
  partition of 2×2×2 blocks, a collision derived from the symmetry of the cube (isotropic,
  momentum-conserving, its own inverse), specular mirror walls, and clear matter (light reading
  above the game's transparency onset) that particles cross without colliding.
- `step`, an exact `unstep` and `reverse` (every velocity at once). The brief's literal rule
  (`Rule::LITERAL`) is kept for comparison.
- Experiments in `src/tests.rs`; benchmark in the physics lab (`laws/margolus/examples/bench.rs`).
- A library: it registers nothing. Requires `pwc-api ^3.1` and `pwc.law-core ^0.1`.
