<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Excitable phase memory

An alternative with explicit local temporal state: a stationary medium can be resting, firing,
or refractory. A single universal neighborhood rule permits signal fronts and delayed reuse of
paths without prescribing block-specific wire or gate behaviors. This is an excitable cellular
state machine, not chemistry or a microscopic energy model.

## State and law

Each of 343 fixed sites has one integer phase in `0..6`: `0` rests, `1` fires, and `2..5` is
refractory. Every complete turn reads the old state everywhere and writes a new state:

```text
phase > 0: phase' = (phase+1) mod 6
phase = 0: phase' = 1 when at least one coupled face neighbor was firing; otherwise 0
```

For configurations A/B, the symmetric contact coupling is
`sum over all a in A, b in B of fit_raw(a,b) >= 0` using the shared integer compatibility
function. An empty sum is zero, so the same inequality also allows vacuum contacts; there are no
named conductors or insulating exceptions. Matter changes can alter coupling while the phase
state remains attached to positions. Coupling is the only composition dependence in this law.

Ctrl+F5 sets the chosen resting site to firing. Input on a refractory/firing site or any amount
other than one is refused, so a pulse does not silently erase its local history. The new domain
starts entirely resting except for that seed. Every injection is an explicit external trigger.
Autonomous advancement of nonzero phases is a designed rule: **no energy, momentum, particle
count or reversible microscopic dynamics is conserved or claimed**. A cyclic phase memory is
already an extra fundamental mechanism; its utility must justify that complexity.

## What to investigate

Track a propagating front, refractory exclusion, multiple pulses colliding, and geometry with
alternative path delays. Excitable-media inspiration suggests rings and reentrant waves, but
this source does not establish that this particular finite cubic rule produces a robust clock,
spiral or useful logic gate. Some seeds simply die after an outward front hits the laboratory
boundary. Absorbing finite boundaries and six-phase memory are research decisions.

Contact compatibility comes from existing matter; if useful coupling is rare
or unintuitive, compare another universal derived interaction formula rather than adding named
wire blocks. The default world palette was tuned for a different passive reaction law. It does
not validate this one's natural material distribution.

## Limits and demonstration

One synchronous phase uses 343 node budget units and at most six neighbor probes per resting
node. Cross-fit evaluation costs O(nA*nB) for each relevant contact, up to 32×32 occurrence pairs.
A future exact cache keyed by immutable configuration pairs may help after profiling; this
prototype makes no throughput claim. A completely resting domain sleeps.

The single focused check seeds air, observes firing at a neighbor and refractory state at the
source after one turn, compares complete restart evolution, and rejects an invalid phase without
replacing state. It proves a local propagation/restart example, not emergent computation.

## Installation and controls

Install this source directory through the PWC package manager: `pwc package check <directory>`,
then `pwc mod add <directory>` in an isolated development instance and `pwc run`. There is no
Cargo manifest in the package: the package manager generates it. Requires `pwc-mod-api ^3.2` and
no other packages. Native code, data and the full source are compiled into the executable; this
package performs no file or network access of its own.

Enable its **enabled** world option, which defaults **off**. Select one physics provider when
comparing laws: the host currently takes the first enabled provider. This package alone is an
independent model and does not compose with the other fundamental-law experiments. A settings
menu or `/set` from another package can change the option; this package does not require a GUI
package. The bare core has no settings screen. Use disposable world/data directories for trials.

In singleplayer, **Ctrl+F5** applies the laboratory input at the player's current canonical cell.
The first accepted input fixes a 7×7×7 patch there. Later input outside this patch is refused.
Stay within three cells in each coordinate to sample the numeric HUD. The HUD honors its hidden
mode. These are numeric laboratories, without a field renderer or a demonstrated useful machine.
No phases/populations become new `BlockId`s, and no existing matter is moved or manufactured.

Input and HUD state are singleplayer only. The factory `spec()` can be installed on an authority,
but clients cannot send these experimental inputs or inspect auxiliary fields over multiplayer:
the generic field snapshot/request protocol is not implemented. Passive simulation uses bounded
integer state and deterministic scheduling. Chunk loading does not initialize a domain. With no
selected provider the model does no work; disabling it freezes its payload rather than deleting
it, and re-enabling resumes the saved state. Saves retain anchor, complete state and counters,
keyed by the package id and dynamics version 1. Invalid saves are rejected before adopting any
state. Switching to a different universe can change its material substrate; keep comparisons
isolated rather than interpreting this as a universal migration policy.

The finite laboratory is a boundary condition chosen for research, not an emergent physical
wall. Cartesian space and one chart interior are supported. Any remapped chart cell or unreadable
cell refuses the whole attempted phase, leaving physical state intact and counting the refusal.
The cell resolver does not rotate direction frames, so larger curved-world domains need a
separate topology contract. A depleted quota pauses a complete phase, rather than exposing a
half-updated lattice. Counters are bounded to 2^60 logical turns; an exhausted clock refuses
further work. `dropped` means refused input/phase, never secretly destroyed resources.

Code, manifest, README, changelog and original content are licensed under **Apache-2.0 OR MIT**,
at your option. Both full texts are included in `LICENSES/`. This initial version is a research
candidate, with no claim of human gameplay validation, large-world throughput or universal
computation. The small source test is intended to run inside a generated development build.
