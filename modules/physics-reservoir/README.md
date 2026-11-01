<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->

# Conservative reservoir laboratory

This candidate keeps the existing configurations but replaces greedy-only reactions with
**affordable local rearrangement**. Every activated cell can hold a nonnegative integer reservoir
`e`; the internal constituent fit supplies a potential. No named temperature, electrical charge,
conductor table, reaction recipe, or material exception is introduced. Energy is an independent
quantity because this experiment specifically tests whether allowing uphill transformations
opens useful possibilities. It is one candidate, not an assumption that PWC needs real-world heat.

## State and law

For a block `A`, let `Phi(A)` be the sum of `fit_raw` over unordered constituent pairs. Interpret
the material potential as `U(A) = -Phi(A)`. A closed collection has total

```
E = sum_i e[i] - sum_i Phi(A[i])
```

An active face contact first balances its reservoirs. If their sum is `s`, the lower cell gets
`ceil(s/2)` and the upper gets `floor(s/2)`. This is exact conservative integer diffusion with a
one-unit orientation bias. It is dissipative information processing, not reversible microscopic
physics. A signal travels through unchanged matter and vacuum; this simple version gives neither
an authored conductivity nor a different transport coefficient for each material.

Then consider transferring one occurrence `x` from donor `A` to receiver `B`. Inserting into a full
32-occurrence configuration is forbidden. Its exact potential reduction is

```
g = sum_{y in B} fit_raw(x,y) - holding_A(x)
```

The move is affordable iff `e[A] + e[B] + g >= 0`. An accepted transfer replaces the two
configurations atomically and deposits exactly `g` into the combined reservoirs, then splits the
result as above. Thus `delta e = delta Phi = g` and `delta E = 0` exactly. A negative `g` is a
potential-increasing move paid from stored energy. There is no normalized minimum-gain threshold;
zero-cost motion is allowed. Empty space is the empty configuration, governed by this same rule.
Occurrence identity and multiplicity are preserved, including duplicates.

Both donor orientations and every donor occurrence are scanned from a rotating deterministic
starting point; the first affordable transfer is chosen. The phase comes from the simulation turn
and contact coordinates. This removes permanent canonical-first preference but does **not**
establish detailed balance, ergodicity, or unbiased thermal equilibrium. This is a deterministic
game-law experiment inspired by microcanonical accounting, not a faithful Monte Carlo sampler.

Swaps are deliberately absent. Two full configurations are jammed under the same capacity rule
as everything else. This may yield useful exclusion/jamming, or may be an unacceptable limitation;
test it before adding another operation. A full configuration can still give an occurrence to a
nonfull neighbor. No current material type is privileged.

## Why explore it

Stored energy can release cohesive matter, turn an otherwise forbidden rearrangement into an
allowed one, diffuse through stationary structures, and be released by favorable recombination.
Player experiments could compare energy pulses against configurations of increasing cohesion,
search for spatially separated energy-consuming and energy-releasing processes, or discover
structures that delay rearrangement. Whether catalysts, cycles, storage, engines, or meaningful
chemistry actually emerge remains unproved. Diffusion without inertia does not offer coherent
oscillation, directed information transport, or lossless clock loops. A potential budget is also
not a complete model of kinetic motion, forces, fracture, or rigid bodies.

The external interface can add **or withdraw** reservoir energy; this represents an explicit
source/sink. Player block placement, removal, or material edits can also change potential through
external work. Conservation claims apply to the module's closed internal steps, not a world being
edited by a player or a generator.

## Run and compare

Install in a PWC 3.2+ build and enable `pwc.physics-reservoir.enabled`, keeping other providers off.
`Ctrl+F3` supplies `4*QUANTUM = 4,194,304` raw energy units to the player's canonical cell. The HUD
reports local/total stored energy and pending work. Move close to a material interface to investigate
diffusion and rearrangement. The injection is local single-player input; connected clients cannot
authoritatively inject it. The root lab can install `spec()` directly. There is no named material
tool-reaction implementation in this experimental provider.

```sh
pwc instance create physics-reservoir-lab
pwc --instance physics-reservoir-lab module add ./modules/physics-reservoir
pwc --instance physics-reservoir-lab build --profile dev --print-dir
# In the generated workspace printed by the builder:
cargo test -p pwc_physics_reservoir --lib -- --nocapture
```

The focused demonstration separates a cohesive two-occurrence block: the transfer is rejected
without energy, accepted with a reservoir, and exactly preserves both the occurrence multiset and
`e-Phi`. This is an algebraic smoke test, not thermal, performance, or gameplay validation.

## Scheduling and resource limits

The ordered deduplicated queue works at most the host's contact quota, 384 attempts per default
turn. Only edits, impulses, energy movement, or accepted transformations wake contacts; generation
and chunk loading do not create simulation state. Each successful internal change wakes neighbors
for later turns. When neither diffusion nor an affordable transfer changes the contact, it becomes
dormant. Since an affordable zero-cost transfer is allowed, some disturbed unbound configurations
may remain active indefinitely. That is a real candidate cost, not evidence of useful chemistry.

There are at most 8192 nonzero reservoir cells and 16,384 pending contacts. Reservoir total is
bounded by `2^60`. State-changing attempts that exceed numeric or storage bounds are refused and
counted, rather than clipped or losing energy. Queue-cap refusals may leave a contact sleeping
until another edit/input wakes it; they need explicit diagnostics and a production overflow
policy. Saved state includes every reservoir and pending arrival turn, validates all bounds and
duplicates, and loads transactionally. There are no per-energy `BlockId`s.

The candidate gain scan costs at most roughly `2*32*32` fit evaluations per contact, plus cached
supports, allocation, and interning on accepted moves. Atomic host pair commits prevent one-sided
matter loss. Failed commits preserve matter and do not receive reaction energy; a valid diffusion
operation earlier in that attempt can still stand. A refused internal operation retains its contact
for a bounded retry on the next turn, so registry/write-capacity failures do not silently lose pending
work. Persistent capacity exhaustion can therefore keep a contact active with counted refusals.
The current intern-table width and churn remain
core architectural limits. A realistic performance study should measure active cascades, registry
growth, cache hits, and queue pressure, rather than just a dormant world.

Cell identity uses the host's topology resolver. Diffusion needs no directional frame rotation,
but interaction-neighbor correctness at chart seams still depends on the host's contact topology.
Energy state is authoritative and saved; field visualization and client telemetry are separate
future instrumentation work.

## Inspirations and original choices

[Creutz and Moriarty's microcanonical Monte Carlo implementation (1983)](https://www.latticeguy.net/mypubs/pub076.pdf)
uses a finite auxiliary energy that pays for potential increases and receives decreases.
That accounting principle motivates this experiment. The module's local reservoirs, PWC constituent
potential, deterministic rotating proposal order, exclusion-only transfers, and exact integer
diffusion are our candidate game rules. They are not a claim that the original sampling algorithm
has been implemented or that equilibrium statistics follow. A future alternative should compare
reversible reservoir transport and symmetrically sampled proposals with this simpler dissipative
version before inferring temperature or heat-engine behavior.

Code: AGPL-3.0-or-later. Documentation: CC-BY-SA-4.0. Existing material primitives retain their
upstream licences; no source from the papers was copied.

## Package management

This source package lives in `pwc-package-manager/modules/physics-reservoir`. `module.toml` is
canonical; the package manager generates Cargo manifests and builds a content-addressed source
snapshot. It depends only on module API 3.2. Add it by id after `pwc setup`, or use
`pwc --instance <lab> module add ./modules/physics-reservoir` from the manager checkout.
Use an isolated test instance; fields are singleplayer experiments, not multiplayer visuals.
The source and documentation retain their existing AGPL/CC licences; complete texts are included.
