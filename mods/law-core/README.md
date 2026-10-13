# Law Core (`pwc.law-core`)

The shared ground of the **candidate fundamental laws**: the packages that propose alternatives to
the game's current reaction law (selective transfer v1). A library: it registers nothing.

Every candidate runs on the same thing, a box of the game's own matter. It must derive every
constant it uses from the one quantity the universe already has: the fit between elements. This
library provides that ground:

- `Grid`: a box of cells and its six face directions (`DIRS`, `opposite`).
- `Matter`: cells holding palette indices (`Mat`, with `VOID` as empty space). The palette holds
  real `material::Block` configurations with cached `Reading`s (occurrence count, cohesion,
  internal fit, and the light/glow/grip probe responses) and the **bond** table. A bond is the
  mean fit across a face between two configurations. Positive means attraction (a spring, a
  coupling, a conductor) and negative means repulsion (an insulator, an inhibition). Readings
  are in 1/256 of `QUANTUM`, the scale of `material::probe_response`.
- `Candidate`: the interface the physics lab (`pwc.physics-lab`) and the experiments drive. It
  covers `step`, the optional exact `unstep`, `poke` (the gameplay verb), `show` (a heat-map
  value), `ledger` (the books), `active`, `state_bytes_per_cell`, `hash_state` and `digest`.
  Equal digests on two peers mean equal futures.
- `Entry`: a candidate's id, name, summary and constructor, so the lab lists them without naming
  any.
- `search`: deterministic helpers (`Stream`, `pure`, `mix`, `find`, `is_cohesive`, `bond`). They
  find configurations with wanted readings, so experiments never hard-code element bytes.
- `show`: ASCII slices and space-time diagrams, used by the experiments and their reports.

Identical elements fit 0, so a block made of one element has cohesion 0 and bonds 0 to itself.
Self-bonding matter is a mixture.

## Licence

`Apache-2.0 OR MIT`, at your option.
