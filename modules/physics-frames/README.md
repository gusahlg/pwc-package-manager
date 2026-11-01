<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Relational frames: matter as transformations

Can one algebraic incompatibility rule provide quiet backgrounds, disturbances, recurrence and
order-sensitive memory? Each site stores a permutation of three abstract markers. The six possible
states form S3, the smallest noncommutative permutation group. They are not six named materials,
and there is no authored collision table. This is a classical fictional-universe experiment,
not a quantum-anyon model or a selected final physics foundation.

## Installation and instruments

Install `pwc module add ./modules/physics-frames` from the package-manager checkout. Enable
`pwc.physics-frames.enabled` in Settings, and disable competing physics providers. It defaults off;
the first enabled provider wins. Use an isolated instance/world. A settings menu or commands
package can expose the option; no GUI dependency is required.

In singleplayer, **Ctrl+F5** conjugates the local frame by (12), preparing a conflict. The first
input prepares a 7×7×7 patch in homogeneous A=(01). This is a laboratory preparation, not an
extra material rule: every uniform frame, and every neighboring commuting mixture, rests.
**Ctrl+F6** cycles the selected cell through the six permutations in lexicographic order. This is
external preparation, not autonomous evolution. Identity cells create inert gaps. The host input
codes are 1=conjugate, 2=cycle, -1=set identity. Input beyond the first patch is refused.

The HUD reports frame rank (0..5; -1 means no prepared cell), noncommuting face edges, committed
pair changes and turns. Walk through the patch to sample neighbors. No field renderer or player
force exists yet. Sources are not closed-system transitions or chemical energy conversions.

## The complete local law

Let ab mean composition: apply b, then a. On an ordered neighbor pair:

```text
if ab = ba:  (a,b) is unchanged
otherwise:   (a,b) -> (b, b^-1 a b)
```

Products compose three indices; inverses reverse their maps. A turn chooses a disjoint face
matching, cycling x/y/z and both coordinate parities. Every encounter reads the same incoming
field. The inverse of an active encounter is `(c,d) -> (c d c^-1, c)`. Commutation/noncommutation
is unchanged by the encounter, so the gate retains exact bijectivity. Reverse phase order and
use inverse encounters to undo fixed-domain evolution; external preparation is outside that proof.

The ordered pair product ab and each conjugacy-class population are conserved. Identity sites
remain fixed individually. A fixed one-dimensional chain preserves its ordered product under
adjacent updates; no comparable global product is asserted in 3D. There is no defined work/heat
ledger, ordinary momentum conservation or independent local gauge symmetry.

With A=(01), B=(12), C=(02), the same law gives:

```text
(A,B) -> (B,C) -> (C,A) -> (A,B)
```

Two encounters restore carrier order but change internal frames. This recurring state was not
implemented as an oscillator recipe. A homogeneous A preparation rests; a B disturbance can
change neighboring sites. Useful signals, junctions and machines are research questions.

## Distinctness and criticism

The earlier matter-permutation model exchanges whole fixed configurations. This law rewrites
internal labels within their conserved classes, with history and compatibility determined by
composition of transformations. It is still a finite reversible cellular automaton.

Ungated conjugation is a standard braid map, but makes carrier paths independent of states.
The commutation gate permits quiet structures while breaking Yang-Baxter: 48 of 216 triples
fail that relation. This intentional tradeoff prevents inheriting quantum braiding, integrability,
protected topology or universal computation claims from the inspiring research.

Identity gaps cannot be crossed; nonidentity support is fixed cell by cell. This is information
dynamics in stationary prepared media, not moving or created voxel matter. Pair orientation
and the six-phase schedule introduce directional bias. Six states may be too restrictive, and
recurrence may be unreadable noise rather than useful clocks. Frames do not yet affect PWC's
constituent reactions, observations or forces. No named gate/wire exceptions repair those limits.

## Cost, persistence and checks

There are at most 343 sites. A whole phase requires a 343-unit quota: 343 host reads, up to 147
pair attempts, and a recount of at most 882 edges. These quota units are sites, not individual
multiplications. Ordered maps and whole-phase scratch state aid review; no world-scale speed
claim is made. A commuting patch sleeps. Domain initialization happens only on explicit input.

Core saves all frames, center and counters as versioned model records. The loader validates
complete cube membership, permutations, duplicates, numeric bounds and clock before adopting
state. Pausing retains the payload. Unsupported chart aliases refuse the whole phase; physical
neighbor/port transformation is pending. Servers can host the factory, but field snapshots and
preparation requests are not replicated, so instruments are singleplayer. There is no package
file/network/process access beyond the API. No Cargo manifest ships: `module.toml` is canonical.

One focused test verifies all 36 pair inverses/products/classes, all 216 triples and their 48 Yang-Baxter
failures, local recurrence, propagation from a prepared source, six-phase reversal, state reload
and transactional corrupt-save rejection. Run it from the manager-generated workspace:
`cargo test -p pwc_physics_frames --lib -- --nocapture`. The managed lab also provides a short run.
These checks establish algebra and bounded propagation, not a technology system.

## Research and fiction

China-affiliated [Xu et al., non-Abelian braiding (2024)](https://arxiv.org/abs/2404.00091) and
[Li, Wang and Zhang, Yang-Baxterization (1994)](https://ctp.itp.ac.cn/EN/Y1994/V21/I2/167)
motivate learning from algebraic encounters; their quantum/matrix systems are not implemented.
[Greg Egan's official Schild's Ladder excerpt](https://www.gregegan.net/SCHILD/00/SchildExcerpt.html)
inspires asking which preparations a different universe can sustain. Fiction is a question source,
not scientific evidence. The gate, finite frames and preparation controls are our chosen toy laws;
no paper or novel code is copied.

## Licence

Apache-2.0 or MIT, at your option; both complete texts are in `LICENSES/`.
