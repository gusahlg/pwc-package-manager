<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Dipole-conserving physics (`pwc.physics-dipole`)

A fictional physical laboratory where mobility follows conserved quantities rather than material
roles. A position remembers only signed occupation `q in {-1,0,+1}`. One local reflection law
preserves total signed charge, its first spatial moment, and the quadratic norm. Consequently, an
isolated unit is frozen, while a neutral pair can translate. There are no declared immobile particle
types, wires, or gates: the difference follows from the same equations acting on different states.
The proposal is an experimental field model, not implemented moving voxel matter or electromagnetism.

## Playing with the laboratory

Install the canonical native package with `pwc module add ./modules/physics-dipole`, rebuild, and enable
`pwc.physics-dipole.enabled` on the World page. It defaults off. Keep other simulation providers off.
The first preparation input establishes a 7×7×7 laboratory centered on the player's canonical cell.

- **Ctrl+F5:** prepare a positive occupation at that cell and a negative one at its +x neighbor.
  Both cells must be empty; the entire preparation is refused if either is unavailable.
- **Ctrl+F6:** prepare one positive unit at the current cell.
- **Shift+F6:** prepare one negative unit at the current cell. Move before adding companions.

The HUD reports local occupation, total charge `Q`, quadratic norm `E`, x dipole moment, steps,
and refused inputs. Walk inside the patch to sample behavior. No field renderer is included.
Preparing charges is an **external source**, which can change the invariants; closed conservation
applies between those inputs. Even a net-neutral preparation changes dipole moment. A source
cannot overwrite an occupied cell. There is no client-authoritative multiplayer injection.
The module requires PWC API 3.2+, has no package dependencies, and opens no files or network connections.

## A single universal interaction

For a local set of positions `r_i`, choose an integer affine-null direction `v`:

```
sum_i v_i = 0
sum_i r_i*v_i = 0
```

These are exactly the conditions that adding `a*v` preserves charge and dipole moment. The simplest
local square-lattice stencils used here are:

- three equally spaced collinear cells, with `v=(1,-2,1)`;
- four corners `[r,r+e_a,r+e_b,r+e_a+e_b]`, with `v=(1,-1,-1,1)`.

These vectors come from geometry, not a particle-pattern table. Both use the **same** transition:

```
a = -2*(v dot q)/(v dot v)
q' = q+a*v
```

If `a` is not an integer, or any candidate occupation is outside `[-1,+1]`, leave the complete
stencil unchanged. Otherwise commit every site together. This is an integer Householder reflection
on its admissible states. It is an involution; refusal also leaves a reversible identity operation.
Its exact closed invariants are

```
Q = sum_i q_i
D = sum_i r_i*q_i                 a three-component Cartesian vector
E = sum_i q_i^2
```

The algebra is direct: `delta E = 2*a*(v dot q)+a^2*(v dot v)=0`, and the affine-null conditions
give `delta Q=delta D=0`. With ternary occupation, `E` equals the number of occupied sites. It is
a conserved norm; calling it an energy is a useful interpretation, not a demonstrated thermodynamic
potential or a coupling to constituent fit.

## Restricted mobility, with necessary caveats

For `E=1`, exactly one site is occupied. Its sign follows from `Q`; its location follows from `D`.
It therefore cannot move under **any** closed evolution preserving these invariants. This is
stronger than dipole conservation alone, which permits motion accompanied by compensating changes
elsewhere. A charge interacting with other excitations can participate in rearrangements; it is
not absolutely immobile in every many-body state.

For a neutral pair `(+1,-1)`, translation does not change `D`, since its total charge is zero.
The triple reflection actually realizes `[+1,-1,0] <-> [0,+1,-1]`, moving the pair along its axis.
The plaquette reflection realizes `[+1,-1,0,0] <-> [0,0,+1,-1]`, moving it transversely. This permits
3D translations of neutral pairs while preserving their separation vector. Broad ergodicity,
arbitrary paths through occupied environments, orientation changes, and useful machines are not
proved. The move set and boundaries can impose further restrictions beyond the three invariants.

The unit bound is fundamental to the demonstrated single-charge claim. Removing it creates a
counterexample: `[3,0,0] -> [2,2,-1]` preserves `Q`, `D`, and norm nine, but splits an isolated
magnitude-three occupation into multiple sites. We therefore do not claim that unrestricted
integer charges are all immobile. This model has one universal occupation capacity, not a table
that specially immobilizes a named excitation.

Possible player experiments include preparing opposite companions to unlock motion, constructing
stable isolated records, and making spatial moment budgets constrain transport. Computation,
resource conversion, and technology remain research questions. The present field does not interact
with block motion, collisions, heat, gravity, or the existing chemical potential. Those couplings
must conserve a coherent combined set of quantities if introduced.

## Scheduling, bounds, and persistence

Each authoritative 20 Hz turn first applies disjoint collinear stencils on axis `turn module 3`, with
starting coordinate residue `floor(turn/3) module 3`. It then applies disjoint plaquettes in plane
`(axis,(axis+1) module 3)`, using the two parity bits of `floor(turn/3) module 4`. The first stage has at
most 98 triple attempts; the second has at most 63 plaquettes. A complete turn therefore costs at
most 161 stencil attempts under the default 384-unit budget. A smaller quota pauses the whole turn
when required; it never commits an arbitrary prefix. Stencil discovery also visits at most 343
sites per stage. Order is deterministic and part of the candidate law, with cubic scheduling
artifacts to investigate.

Within each stage the stencils are disjoint. To invert a turn, repeat the plaquette stage first and
then the collinear stage, using that turn's original phase. Every transition is an admitted bounded
integer reflection. `Q` and `E` are at most 343 in magnitude; each component of `D` is bounded by
`343*2^31`, so exact `i64` diagnostics remain safe even near integer world-coordinate limits.

The host persists every ternary occupation, domain position, turn, and counter. Loading validates
numeric bounds, duplicates, complete seven-cell-cube geometry, and finite clock before replacing
anything. No field phase becomes a material `BlockId`. Inputs outside the original patch, chart
aliases, unreadable cells, and exhausted clocks are refused or pause with diagnostics. The clock
pauses before committing `u64::MAX`, so every produced clock value can be restored by the loader.
Outside links are absent. Dipole moment is defined on this fixed Cartesian patch, not on wrapped
charts, a periodic torus, or moving origins. No whole-world regional scheduler is claimed.

The public `Dipole::inject_charge` helper is an explicit unit-charge source for direct laboratories;
`invariants()` returns all three components of `D`. The host's `perturb` input is a documented
source-selector protocol: `+1/-1` prepares an oriented neutral pair; `+2/-2` prepares a positive or
negative unit. Other selectors are refused. These preparation controls are separate from the
single internal reflection law.

One focused demonstration leaves an isolated unit unchanged through 36 scheduled phases, translates
a neutral pair both longitudinally and transversely, checks exact `(Q,D,E)` after each transition,
and reverses the pair moves. This establishes those local algebraic and mobility claims, not broad
performance, gameplay, quantum fracton order, or a complete simulated universe.

## Primary inspiration and independent proposal

[Begun, Chernodub, Goy and Molochkov, *Vortex wall phase in the fractonic XY-plaquette model on a
square lattice* (2025)](https://journals.aps.org/prb/abstract/10.1103/y3yg-zkzv) studies restricted
mobility in a different statistical lattice model. The published author affiliations identify
Molochkov with the Beijing Institute of Mathematical Sciences and Applications, Tsinghua University,
China. An [author preprint](https://arxiv.org/abs/2409.15638) is available. It provides inspiration
for making mobility an outcome of constraints rather than a named material behavior.

This package does **not** implement that paper's imaginary-time XY dynamics, Monte Carlo phases,
or vortex-wall results. The ternary affine-null Householder rule, norm-based immobility proof, and
explicit longitudinal/transverse preparation experiments are this candidate's own proposal. No
implementation code was copied, and the algebra above must stand independently of the inspiration.

## Licence

Code, manifest, documentation and assets are licensed under either the Apache License, Version 2.0
or the MIT licence, at your option (`Apache-2.0 OR MIT`). Both full texts are in `LICENSES/`.
