<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Digital spin physics (`pwc.physics-spin`)

An experimental physical universe with a binary orientation and a finite local energy bin at each
activated position. Neighboring orientations interact under one rule. Energy can change collective
order without changing or naming block configurations. The proposal explores memory, switching,
domain interfaces, and transmitted disturbances rather than adding a catalogue of electrical parts.
These possibilities are research questions; no useful circuit, phase transition, or gameplay loop
has been established.

## Playing with the laboratory

Install this native source package with `pwc module add ./modules/physics-spin`, rebuild, and enable
`pwc.physics-spin.enabled` on the World settings page. It defaults off. Enable one simulation
provider at a time. `Ctrl+F5` adds 64 external energy units to the player's canonical cell; the first
input activates a seven-cell cube centered there. The HUD reads local orientation and stored energy.
Walk within the cube to sample neighbors. There is no spin renderer yet.

Input is single player; a multiplayer client does not inject authoritative energy. Ordinary block
edits do not activate another patch or replace the frozen coefficients. This prototype supplies no
held-material reaction. It reads the host's material configurations, has no package dependencies,
and requires PWC module API 3.2 or later. It performs no file or network access itself.

## Fundamental state and laws

Each cell remembers `s in {-1,+1}` and `e in [0,4096]`. Its initial coupling scale is
`j = 1 + floor(n/8)`, where `n` is the occurrence count of the local configuration. On a face edge
`J_ij = min(j_i,j_j)`. These numbers are frozen and saved when the laboratory first advances; there
is no named magnetic material or hidden conductivity table. Every site begins aligned with zero
reservoir energy. Vacuum participates under the same law with scale one.

The closed energy is exactly

```
E = sum_i e_i - sum_<i,j> J_ij * s_i * s_j
```

Flipping one orientation changes the interaction potential by
`cost = 2*s_i*sum_j(J_ij*s_j)`. Accept the flip precisely when `e_i-cost` lies in `[0,4096]`, changing
both orientation and reservoir atomically. Otherwise leave both unchanged. This local operation is
an involution and preserves the integer energy exactly, including at the reservoir bounds.

A complete turn visits sites in lexicographic order, then swaps entire neighboring reservoir bins
on a disjoint matching. Matching axis is `turn module 3`; pairing parity is `floor(turn/3) module 2`.
This provides conservative deterministic energy transport. It is a swap permutation rather than a
heat equation or a proven diffusion process. To invert a turn, undo the matching and visit spin
flips in reverse order. Fixed graph and coefficients are required; an external energy input changes
the closed energy and is deliberately separate from conservation claims.

The only authored behaviors are these general rules, initial conditions, and the experiment's
boundary. Orientation is independent of the existing constituent potential: there is no claim
that spin energy is paid by selective-transfer chemistry. That coupling would require a coherent
combined energy law.

## Scope, cost, and persistence

The first input owns a 7×7×7 patch, at most 343 sites, with reflecting/noninteracting outside edges.
Further input outside it is refused and counted. A quota smaller than its site count pauses the
whole turn. Each quota unit covers one site's at-most-six neighbor reads and its share of the
matching pass. The model therefore has bounded `O(V+E)` work; it is not 343 independent contact
attempts. Ordered maps prioritize clarity over maximum throughput. A production spin lattice could
use packed arrays or bit parallelism, after meaningful behavior is demonstrated.

The simulation host persists orientation, reservoir values, frozen scales, initialization state,
and counters. Loading validates a complete seven-cell cube, finite numeric bounds, and duplicate
positions in a temporary state before adopting anything. Exhausted clocks, unreadable cells, or
chart aliases pause with a refusal count. No field phase consumes a material `BlockId`. The small
Cartesian laboratory does not support cross-chart transport or a whole-world regional scheduler.

The one focused demonstration evolves an excited three-site system, checks exact closed energy
after every sweep, then inverts eight sweeps and recovers the original state. It checks algebra;
it does not establish thermal equilibrium, ergodicity, useful memory retention, or performance at
world scale. No Landau free-energy functional or exact transition temperature is asserted. The
lexicographic sweep and six-phase matching can create directional artifacts and should be compared
with checkerboard scheduling before interpreting large-scale behavior.

## Inspiration and next experiments

[Creutz, *Deterministic Ising Dynamics* (1986)](https://latticeguy.net/mypubs/pub088.pdf) demonstrates
energy-conserving orientational dynamics using local integer auxiliary variables. This proposal
uses a different sequential flip schedule, count-derived couplings, and explicit bin swaps. It does
not inherit that paper's measurements. No implementation source was copied.

Compare an excited spot inside an aligned domain with an interface between opposite orientations.
Look for readable switching thresholds, propagation, long-lived memory, or periodic states.
Reject or revise the model if those are inaccessible, if signal noise dominates, or if useful
behavior requires prebuilt gates. Adding more spin directions may help, but requires evidence.

## Licence

Code, manifest, documentation and assets are licensed under either the Apache License, Version 2.0
or the MIT licence, at your option (`Apache-2.0 OR MIT`). Both full texts are in `LICENSES/`.
