# Changelog — pwc.law-zeldovich

## 0.1.0

- First release. The synthesis law: one exactly conserved integer energy
  (`Σ quanta − Σ binding − work`). A face rule moves one occurrence across a face when the quanta
  striking it pay its barrier and its binding change (selective transfer's ranking, its threshold
  in both directions). A cell rule deals the quanta entering a cell over its other conducting faces,
  with a rotor-router for remainders. Experiments in `src/tests.rs`, bench in the physics lab.
- A library: it registers nothing. Requires `pwc-api ^3.1` and `pwc.law-core ^0.1`.
