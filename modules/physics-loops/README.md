<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Integer circulation on loops

This candidate stores memory on **edges**, rather than inside material configurations or scalar
voxel fields. It asks whether local conservative constraints can preserve and redistribute
closed circulation through one universal reversible update, with no named wire or gate types.
It is inspired by discrete circulation and plaquette operations, but does not implement a
physical electromagnetic field or a lattice gauge theory.

## State and exact local law

Each positive-axis edge inside the 7×7×7 domain remembers a signed integer current `J`; zero
edges are omitted. State is restricted to zero discrete divergence at every vertex. Ctrl+F5
adds an explicit circulation of magnitude 16 around the positive XY square anchored at the
player cell. Signed extra inputs through the model API are external circulation sources; all
four affected edges are validated and adopted together. The positive X/Y square must fit inside
the anchored patch, so input on its positive boundary may be refused. No endpoints or source/sink
charges are silently created.

For any square face with consistently oriented boundary currents
`f=(Jx(bottom), Jy(right), -Jx(top), -Jy(left))`, apply

```text
q = floor((f0+f1+f2+f3)/4)  [Euclidean integer division]
f_i' = f_i - 2q            [all four boundary currents]
```

This adds the same signed circulation to the face boundary. At each corner the incoming and
outgoing changes cancel, so vertex divergence stays exactly zero. If the sum is `4q+r`, with
`0<=r<4`, its new sum is `-4q+r`; the next application has quotient `-q` and restores every
integer bit. The update is therefore an **involution**, including negative currents.

One logical turn applies disjoint faces from one of 12 deterministic phases: XY/YZ/XZ planes,
each split by both lower-coordinate parities. Reversing the sequence of phases restores a
closed evolution on an unchanged domain, provided the same refusal decisions are used. A whole
phase that would exceed any current limit is refused as identity, not clipped; the involution
maps accepted bounded states in pairs. Counters and external inputs are not part of this inverse.
Overlapping faces interact on later phases and redistribute circulation beyond one square.

## What is and is not conserved

Divergence and the integer state under the explicit inverse are exact. The quadratic current
norm is conserved when a face sum is divisible by four, and by identity steps with zero quotient.
Other quantized remainders can change that norm: **this model does not claim general energy or
momentum conservation**. It preserves the no-endpoint constraint, not every loop's shape,
length, orientation, knot class or circulation amplitude.

All loops in this simply connected finite box are contractible. There is no protected winding
number, no demonstrated topological qubit, and no gauge equivalence quotient: the edge currents
are directly observable state, rather than a potential whose gauge changes leave all observations
unchanged. Calling it gauge-like only refers to local closed-boundary updates and constrained
state. Establishing topological memory would require a different topology and explicit invariant.

## What to investigate

Compare overlapping injected squares, undo complete phase sequences in the headless workbench,
and search for recurrent circulation patterns. Is edge memory more expressive than a scalar
phase for the same cost? Can an external read/write interface expose useful processes without
making named gates? Matter currently does not change the plaquette update; the first comparison
holds the geometry fixed. This law is not yet a whole-world force or material binding model.

The domain has at most 882 positive edges, each current within [-4096,4096]. A phase attempts at
most 63 disjoint faces; budget units count face attempts, plus bounded domain validation and
scratch-map costs. Empty current state sleeps. Save loaders validate bounds, duplicate edges,
anchor containment and zero divergence before adoption. The focused test applies all 12 phases,
checks divergence, reverses them exactly, reloads and rejects a lone open edge. It demonstrates
those algebraic/persistence facts, not protected topological storage or game-scale performance.

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
