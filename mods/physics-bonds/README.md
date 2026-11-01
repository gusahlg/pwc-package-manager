<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Plastic bond physics (`pwc.physics-bonds`)

An experimental elastic network in which geometry has memory. Neighboring sites share bonds;
displacing one site changes every incident strain and makes neighboring sites respond. Excess
strain changes a bond's learned rest value. The resulting memory is a universal plastic response,
not a catalogue of springs, hinges, or named material behaviors. This candidate explores stress
transport, yielding, hysteresis, and dissipation, complementing the momentum-wave and purely
chemical-reservoir models.

## Playing with the laboratory

Install with `pwc mod add ./mods/physics-bonds`, rebuild, and enable `pwc.physics-bonds.enabled` on
the World settings page. It defaults off. Keep competing simulation providers off. `Ctrl+F5`
externally raises the player's canonical site's scalar displacement by 32; the first input creates
a seven-cell cube. The HUD shows displacement and local accumulated heat. Walk inside the domain
to sample transmitted responses. No field renderer, held-material reaction, or moving block is
included yet. In particular, **these displacements do not apply forces to the player or integrate
voxel positions**: this is a coarse elastic-network laboratory, not implemented rigid-body physics.

Input is single player. Clients cannot authoritatively displace a multiplayer network. The mod
requires API 3.2+, has no package dependencies, and opens no files or network connections itself.

## Fundamental state and laws

Each site remembers scalar displacement `u` and accumulated nonnegative heat `h`. Every face edge
remembers an integer reststrain `r_ij`. For its canonical lower/upper endpoints,

```
strain_ij = u_j-u_i-r_ij
V = sum_<i,j> strain_ij^2
E = V + sum_i h_i
```

Reststrain starts at zero. At the first advancement each site's yield threshold is observed and
frozen as `y_i = 2+floor(n_i/4)`, using its material occurrence count. Each edge uses
`min(y_i,y_j)`. All configurations, including vacuum, obey the same formula. Count is deliberately
the only material reading in this initial experiment; there is no elastic-material lookup table.

First visit sites in lexicographic order. For each site, evaluate the force-like integer
`F = sum(outgoing strain)-sum(incoming strain)`, and let `d` be its degree. A one-unit displacement
in direction `sign(F)` changes potential by `d-2*abs(F)`. Perform it only when this is strictly
negative, depositing exactly `2*abs(F)-d` heat at that site. A shared site's movement changes its
other incident strains, so this is an interacting network that redistributes stress. It is an
overdamped energy descent, not an inertial oscillator.

Then visit edges. Whenever `abs(strain)>yield`, shift the learned reststrain one unit toward the
current displacement difference. That reduces the squared potential by `2*abs(strain)-1`; deposit
exactly that amount of heat at the canonical lower endpoint. The reststrain persists after the
external load is removed. This is plastic memory and hysteresis rather than a phase encoded as a
different material configuration.

For a fixed graph, every internal operation decreases `V` and increases `h` by precisely the same
integer amount: **closed `E` is conserved exactly**. Mechanical activity dissipates into stored
heat, and this update is intentionally irreversible. Heat does not feed back into yielding or
diffuse yet. Calling it heat identifies an energy ledger, not an established temperature. The
local descent and creep rules have no static recipes or named spring blocks.

An external displacement can add or withdraw elastic energy. The mod records its exact potential
change in a work ledger, which must agree with total energy starting from the initially unstressed
network. Repeated input can therefore expose how much work became permanent memory and heat;
conservation claims exclude that external work. Material edits leave frozen thresholds unchanged.

## Scope, cost, and persistence

The first input owns a complete 7×7×7 patch, 343 sites and 882 internal face bonds. Outside links
are absent. One quota unit covers a site and its bounded share of the displacement and three-edge
creep passes. A quota below its site count pauses the whole step. Work is `O(V+E)` with small
integer operations; allocations and ordered maps are prototype costs. The lexicographic update
can bias relaxation, so alternative scheduling deserves a controlled comparison.

Displacements and reststrains are bounded by ±4096; each site's heat is at most `2^50`. These
bounds keep all strain, force, and energy calculations inside explicit safe integer ranges. An
internal bound/capacity failure rolls the entire phase back and increments refusals. Nothing is
clipped or silently dissipated out of the ledger. An outside input, unreadable cell, chart alias,
or exhausted clock is refused. The laboratory does not rotate geometry across charts or provide
a whole-world region scheduler.

Saved state contains displacements, heat, reststrains, frozen yields, initialization flag, exact
work, and counters. Loading validates numeric ranges, the complete cube, duplicates, and zero
reststrain where no bond exists; it also verifies `work == V+sum(h)`. A temporary state is adopted
only after every check succeeds. No field phase consumes a material `BlockId`.

One focused demonstration displaces a three-site chain, verifies that a neighboring site responds,
that yielding leaves reststrain memory, and that elastic-plus-heat energy remains exact over six
steps. It checks these rules, not full mechanics, fracture, fluid flow, useful machines, or player
experience. Scalar displacement cannot represent rotations or general 3D strain. The immobile
substrate and absence of heat feedback are major limitations, not solved features.

## Inspiration and next experiments

[Nicolas, Ferrero, Martens and Barrat's review of elastoplastic models](https://arxiv.org/abs/1708.09194)
describes how local relaxation and stress redistribution can produce collective material behavior.
This proposal does not implement their quantitative continuum kernels or inherit their results.
Its deliberately simple squared edge potential, one-unit site descent, count-derived yielding,
and explicitly conserved heat ledger are candidate game rules. No implementation source was copied.

Compare repeated load/unload cycles, branch networks, and loops; measure residual reststrain and
how far stress travels. Ask whether deformation memory can act as a readable latch without
authored gates. Before integrating forces into gameplay, add vector displacement and test rotational
consistency, topology changes, work accounting, and renderer/collision coupling. Fracture should
follow an energy budget if introduced; a block-type “break threshold” would weaken this proposal.

## Licence

Code, manifest, documentation and assets are licensed under either the Apache License, Version 2.0
or the MIT licence, at your option (`Apache-2.0 OR MIT`). Both full texts are in `LICENSES/`.
