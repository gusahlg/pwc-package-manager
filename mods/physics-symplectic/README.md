<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Modular symplectic physics (`pwc.physics-symplectic`)

An experimental universe of finite angular phases and momenta, nonlinear local interactions, and
composition-derived frustration. Unlike the unbounded displacement-wave laboratory, every state
here lives on an integer torus. Wrapping is the defined mathematics, not an overflow workaround.
The aim is to explore synchronization, nonlinear signal mixing, recurrent motion, and competing
local preferences with two compact quantities. Useful machines and chaos remain hypotheses.

## Playing with the laboratory

Install with `pwc mod add ./mods/physics-symplectic`, rebuild, and enable
`pwc.physics-symplectic.enabled` on the World page. The option defaults off. Keep other model
providers off. `Ctrl+F5` adds external modular momentum 2048 at the player's canonical cell and
activates a seven-cell cube on the first input. Walk within the domain to read local phase and
momentum on the HUD. Values range from 0 to 65535 and should be read as angles, not signed speeds.
There is no phase renderer, rigid-body motion, or held-material reaction.

The input is local single player; multiplayer clients cannot authoritatively inject it. The mod
requires PWC API 3.2+, has no package dependencies, and opens no files or network connections itself.

## Fundamental state and laws

Every activated cell remembers `q,p in Z/65536Z`. At initial advancement it observes the sum of each
of the first three element resource coordinates, then freezes
`w_i[axis] = 64*sum(element_coordinate[axis]) mod 65536`. The fourth resource coordinate is not used
by this particular experiment. Each positive-axis face edge gets an oriented preferred offset
`alpha_ij = w_i[axis]+w_j[axis] mod 65536`. The inverse edge has `-alpha_ij` by convention; code
visits each canonical edge once. Vacuum has zero offsets.

Define an odd periodic triangle force, for `x` represented in `[0,65535]`:

```
T(x) = x          for x < 16384
       32768-x    for 16384 <= x < 49152
       x-65536    otherwise
```

Its sign and derivative change around the circle; this is not a linear spring. Each edge performs
the momentum shear

```
f = trunc(T(q_j-q_i-alpha_ij) / 4)
p_i += f; p_j -= f                 all momentum arithmetic modulo 65536
```

All kicks read old phases. Then every site applies `q_i += p_i mod 65536`. Invert by subtracting the
drift first and undoing edge kicks in reverse order. Every shear is a translation conditioned on
the other variable; thus the map is **exactly bijective bit for bit** at fixed coefficients. The
sum of momentum is conserved modulo 65536. Neither the signed ordinary momentum sum nor a
physical quadratic energy is claimed to be conserved. External input adds an explicitly supplied
modular momentum, including negative amounts through exact residue reduction.

The continuous piecewise-linear analogue can be associated with a periodic pair potential; its
edge contributions have a symmetric force Jacobian away from corners. Quantized rounding and
finite modular state do not automatically prove a canonical continuous symplectic structure. The
package name refers to the kick/drift inspiration; its proven claim is finite-state reversibility,
not a general symplectic theorem. Nonzero circulation of preferred offsets around a loop creates
frustration because local preferred phases cannot all be met simultaneously. Homogeneous vacuum
with all phases/momenta zero remains quiescent.

## Scope, cost, and persistence

The finite patch contains 343 nodes and 882 internal edges. Outside links are absent. One budget
unit counts a site and its bounded share of the whole kick/drift phase; quotas below 343 pause it.
The arithmetic is fixed-size integer operations with no sine calls or floating-point decisions.
Ordered maps, edge-vector construction, and source-domain reads remain prototype overhead. Native
compilation avoids interpreted hooks per voxel, but this is not a world-scale performance claim.

The host saves all angular state, frozen twists, initialization flag, and counters. Transactional
loading rejects malformed integers, duplicate positions, domains other than the complete
seven-cell cube, and invalid initialization state. There are no phase-specific `BlockId`s. Chart
aliases, unreadable cells, and exhausted clocks pause with counted refusals; the prototype has no
frame transform or topology-aware graph. Further input outside the original patch is refused.
Block edits inside it leave the frozen twists unchanged, preserving a well-defined laboratory law.

One demonstration starts with nontrivial phases, momenta, and frustration, advances 32 steps across
modular wrap boundaries, verifies the modular total, and reverses all steps exactly. That establishes
reversibility. It does not establish positive Lyapunov exponents, a useful clock, arbitrary
computation, a thermodynamic arrow, or enjoyable gameplay. Finite state guarantees eventual
recurrence, but neither its period nor accessibility has been measured.

## Inspiration and next experiments

[Chirikov and Shepelyansky's account of the standard map](https://www.scholarpedia.org/article/Standard_map)
explains nonlinear kicked maps and how regular and chaotic trajectories can coexist. This mod uses
integer angular variables, a triangle force, pairwise equal/opposite kicks, and material-coordinate
offsets instead of reproducing that map or its established chaos thresholds. No source code was
copied. Frustration and two-variable modular dynamics are the research choices being tested.

Compare neighboring trajectories separated by one phase unit, look for repeatable synchronized
domains, and measure mixing before asserting chaos. Compare zero offsets with a loop whose
preferred phases conflict. A future player tool should display phase relations and spectra; useful
interference, memory, or nonlinear switching must arise from constructions rather than authored
gate blocks. If modular momentum lacks a readable energy interpretation, this model may fit an
information substrate better than a complete physical foundation.

## Licence

Code, manifest, documentation and assets are licensed under either the Apache License, Version 2.0
or the MIT licence, at your option (`Apache-2.0 OR MIT`). Both full texts are in `LICENSES/`.
