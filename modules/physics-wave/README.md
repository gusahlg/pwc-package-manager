<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->

# Integer wave laboratory

This native module explores **two integer quantities and two universal shears** instead of adding
electricity, a conductor catalogue, or named machine behaviors. It is a working finite field
experiment, not a proposed final universe. Matter keeps its current element configurations; each
activated position has a separate displacement `q` and momentum `p`. Phases never become new
`BlockId`s. The field does not yet move voxels or act on player collision.

## Laws

The first explicit impulse activates a 7×7×7 Cartesian patch. A cell's mass is `m = 1 + n`, where
`n` is its configuration's occurrence count. Vacuum has mass one; all matter follows the same rule.
Every internal face edge `(i,j)` computes the signed, toward-zero rounded force

```
f = trunc((q[j] - q[i]) / 16)
p[i] += f
p[j] -= f
```

The momentum shear reads the old displacement everywhere. Then every cell performs

```
q[i] += trunc(p[i] / (8 * m[i]))
```

Patch edges have no outside links: an explicit reflecting boundary. This is a laboratory boundary,
not a claim about the shape of PWC's universe. Geometry and masses are sampled once before the
whole step. A terrain edit changes inertia through occurrence count; no material-specific table
enters the law. An impulse adds external momentum at one cell and is not a conservation test.

For fixed masses and topology, invert the drift first and the edge kicks in reverse order. Both
are integer translations conditioned on the other variable, so rounding does not erase the state
needed for inversion. Intermediate arithmetic uses checked `i64` state and `i128` differences. An
overflow refuses the **entire** transition, preserves the old state, and increments `dropped`.
Momentum is exactly conserved by internal kicks. A summed HUD diagnostic may clip to fit an `i64`
readout; the stored physical state does not clip. Reversibility alone does not conserve an ordinary
quadratic energy: this distinction matters when comparing this model with the reservoir candidate.

Ignoring integer rounding, the kick/drift system has the usual oscillatory small-amplitude modes.
Its uniform-mass cubic graph has `lambda_max <= 12`; the normalized step coefficient is at most
`12/(16*8)`, safely below the linear stability boundary of four. This only establishes the linear
idealization. It does not prove bounded energy under indefinitely repeated quantized steps. Net
momentum excites a drifting zero mode; resonant motion is better measured relative to the mean or
with equal and opposite impulses.

## Interesting experiments

- Build regions of different occurrence counts and compare delay and reflected amplitudes.
- Compare a compact pulse, a zero-momentum dipole, and a periodically driven source.
- Search for standing waves, interference-based sensing, cavities, and material identification by
  timing, rather than declaring wires, sensors, oscillators, or recipes.
- Ask whether one simple nonlinear shear based on strain could support switching or localized
  excitations. Do not add it until a minimal test shows something the linear field cannot do.

The present linear field cannot by itself offer general switching, work-to-matter conversion,
fracture, bulk mechanics, persistent useful amplification, or a thermodynamic arrow. Discoverable
resonance is plausible, but player-readable instrumentation and useful machine couplings remain
unbuilt. The element model only supplies inertia, so equally populated materials are dynamically
indistinguishable in this particular candidate.

## Run and compare

Install the source package into a PWC 3.2+ build through the package manager and turn on
`pwc.physics-wave.enabled` in the options registry. Keep other model providers off. `Ctrl+F6`
injects momentum 4096 into the player's canonical cell; the HUD samples that cell. Move inside the
patch to sample nearby responses. Laboratory input is local single-player input; a multiplayer
client does not authoritatively inject it. The root simulation lab can install `spec()` directly.

The first seed owns the domain; impulses outside it are refused and counted. A chart seam or
unreadable cell pauses the experiment with a counted refusal. Scalar state could eventually cross
charts, but this prototype does not build a topology-aware graph. Ordinary chunk loading creates
no field state. There is no field renderer yet, just the numeric HUD and headless demonstration.

```sh
pwc instance create physics-wave-lab
pwc --instance physics-wave-lab module add ./modules/physics-wave
pwc --instance physics-wave-lab build --profile dev --print-dir
# In the generated workspace printed by the builder:
cargo test -p pwc_physics_wave --lib -- --nocapture
```

The focused demonstration propagates an impulse across three stationary cells, checks exact
momentum conservation, then reverses 24 steps and recovers every integer bit. It is a kernel
demonstration, not gameplay or scaling validation.

## Budget and continuation

The domain holds at most 343 nodes; one whole step needs one budget unit per node. A default
384-unit turn can complete it. A smaller quota pauses the entire step, rather than splitting a
wave phase and changing the law according to processing order. Work is `O(V+E)` with `E < 3V`,
plus allocation and ordered-map costs in this prototype. Saved state includes the complete sparse
domain, phase variables, and counters, with strict bounds and duplicate rejection. It does not
allocate anything for unactivated world regions. This experiment does not yet satisfy the cost
goal of large networks: compact regional arrays, sparse activation, and a carefully defined
region/clock scheduler need measurement before expanding it.

## Inspirations and original choices

[Margolus, *Physics-like models of computation* (1984)](https://fab.cba.mit.edu/classes/862.22/notes/computation/Margolus-1984.pdf)
demonstrates discrete reversible dynamics by retaining enough temporal state.
[Rein and Tamayo, *JANUS* (2017)](https://arxiv.org/abs/1704.07715) shows why exact integer
reversibility can matter in numerical physics. This module does not implement either paper's model.
The proposed PWC experiment combines a count-derived inertial field, a finite transparent lab
budget, equal/opposite rounded edge kicks, and an invertible local drift. These are research
choices to challenge, not claims of novelty or proof of rich emergent gameplay.

Code: AGPL-3.0-or-later. Documentation: CC-BY-SA-4.0. No source from the papers was copied.

## Package management

This source package lives in `pwc-package-manager/modules/physics-wave`. `module.toml` is
canonical; the package manager generates Cargo manifests and builds a content-addressed source
snapshot. It depends only on module API 3.2. Add it by id after `pwc setup`, or use
`pwc --instance <lab> module add ./modules/physics-wave` from the manager checkout.
Use an isolated test instance; fields are singleplayer experiments, not multiplayer visuals.
The source and documentation retain their existing AGPL/CC licences; complete texts are included.
