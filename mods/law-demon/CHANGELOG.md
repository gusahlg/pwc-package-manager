# Changelog — pwc.law-demon

## 0.1.0

- First release. The microcanonical law: per-cell heat, an exactly conserved energy ledger
  (`Σ heat − Σ binding − work`), conduction from positive bonds, and one move rule. Each contact,
  on a six-phase dimer schedule, makes the best selective-transfer move that each cell can pay for
  from its own heat, so activation and per-cell energy conservation follow from it. Experiments
  are in `src/tests.rs`, and the bench is in the physics lab.
- A library: it registers nothing. Requires `pwc-api ^3.1` and `pwc.law-core ^0.1`.
