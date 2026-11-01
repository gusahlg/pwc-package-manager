<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Finite fuel and catalytic fronts

A deliberately small alternative to passive compatibility ascent: a self-amplifying signal is
paid for by finite local feedstock. It asks whether a common microscopic resource rule can make
ignition, front propagation and exhaustion legible to players without named fuel blocks or
crafted logic gates. Composition is not yet coupled to these auxiliary populations; that is an
explicit limitation of this candidate, not a hidden material behavior table.

## State and law

Every patch site remembers three nonnegative integer populations: feedstock `F`, active catalyst
`A`, and spent population `W`. One full turn first transports at most one `A` on each selected
face edge from the higher population to the lower, when their difference exceeds one. Edges form
a disjoint matching: X/Y/Z orientations, each with alternating lower-coordinate parity, cycling
through six phases. There is no clipping, stochastic tie-breaking or scan-order update.

Then every site applies the same local rule:

```text
A > 0 and F > 0: (F,A,W) -> (F-1,A+1,W)   [F+A -> 2A]
A > 0 and F = 0: (F,A,W) -> (F,A-1,W+1)   [A -> W]
A = 0:          no local conversion
```

Each rule and transport conserves `F+A+W` exactly. It is a particle/resource accounting law, not
a claim that this scalar is physical energy, or that momentum and entropy are conserved. There
are three universal auxiliary states; there are no per-material reaction recipes. Populations
remain in the same cells; transport changes their counts without moving visible voxel matter.

The first Ctrl+F5 input **externally loads eight feedstock tokens at each of 343 sites**, then
adds one catalyst at the input cell. Its initial finite budget is therefore `343*8+1 = 2745`
tokens. Every later accepted Ctrl+F5 input adds exactly one external catalyst and changes the
total by one. Positive amounts through the model API add that many explicit source tokens. These are explicit player/workbench sources, not spontaneously replenishing fuel. After
fuel is consumed, conversion creates spent tokens, never deletes them. There is no spent-to-fuel
recycling rule, and no infinite-fuel claim. At most 2^20 tokens exist in one laboratory; excess
input is refused completely. Per-site arithmetic is checked and a failed turn adopts no counts.

## What to investigate

A front can multiply locally while feedstock remains and can die after reaching depleted regions.
Watch adjacent cells as catalyst spreads; compare injection times and locations against local
feed/spent readouts. Rate, lattice orientation and the finite boundaries are part of the law.
A count difference of one does not transport, so low populations have a visible deadband.
This is not a guarantee that one seed reaches every site or that all fuel is consumed.

Potential next comparisons include composition-dependent transport derived from the resource
lattice, reversible conversion with a separate stored-work accounting variable, and multiple
fronts meeting in depleted channels. Any such changes need a new law version and evidence, not
named-material patches. The present feedstock loading is an experiment fixture, not natural
world generation or a complete material chemistry.

## Limits and demonstration

A complete phase costs one budget unit per site (343), plus at most one matching neighbor per
site and one local conversion. Default quota 384 admits it. The ordered map and whole-state
scratch copy are bounded prototype costs, not an optimized massive-world representation.

One focused test evolves a finite seed for 96 turns, checks the token total after every turn,
observes spent population, reloads and compares the next complete state, and confirms that a
malformed incomplete domain leaves the loaded state unchanged. This proves those finite
accounting/restart properties, not the existence of useful autonomous technologies.

## Installation and controls

Install this source directory through the PWC package manager: `pwc package check <directory>`,
then `pwc module add <directory>` in an isolated development instance and `pwc run`. There is no
Cargo manifest in the package: the package manager generates it. Requires `pwc-api ^3.4` and
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
