# Changelog — pwc.law-belousov

## 0.1.0

- First release. The excitable medium: one phase byte per cell, one synchronous threshold rule
  whose weights are the bonds (positive excites, negative inhibits), the firing line at the
  game's emission onset (so exactly the light-emitting blocks are pacemakers) and the refractory
  length from cohesion. Sparse: a world at rest costs nothing per tick. Experiments in
  `src/tests.rs`, bench in the physics lab.
- A library: it registers nothing. Requires `pwc-api ^3.1` and `pwc.law-core ^0.1`.
