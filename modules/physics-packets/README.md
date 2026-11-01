<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->

# Conservative packet laboratory

This candidate remembers **six directional occupation bits** at an active cell and applies a
nonlinear conservative scattering rule followed by transport. Signals move through stationary
material without transforming its configurations. The state is distinct from material intern
identities: an occupied signal channel never consumes a `BlockId`. It is a working local lattice
experiment, not an established model of computation or fluid mechanics.

## Laws

Channels are `(+x,-x,+y,-y,+z,-z)`. Each channel holds at most one indistinguishable packet. A local
axis `c` is obtained from the sum of all element coordinates modulo three; vacuum uses zero. This
is one formula for every configuration, not a catalogue of circuit blocks. The other two axes are
`a=(c+1)%3` and `b=(c+2)%3`.

The complete scattering law is:

> If exactly one control-axis channel is occupied, and one other axis holds a head-on pair while
> the remaining axis is empty, exchange which axis holds the pair. Otherwise leave all channels
> unchanged.

The spectator/control channels never change in the scattering step, so this map is an involution
on all 64 masks. It exactly conserves packet count and local vector momentum: a head-on pair has
zero net momentum whichever axis carries it. The spectator makes the interaction nonlinear;
superposing an additional packet can route a pair differently. No named gate, wire, or mirror is
registered. Calling a spectator a “control” is an observation about the mathematics, not a
special class of matter.

Each resulting packet moves one cell along its channel when the interface is open. Both voids
have an open interface. An occupied/void interface reflects. Two nonempty configurations have an
open interface exactly when the sum of their constituent pair fits is nonnegative:

```
open(A,B) = sum_{a in A,b in B} fit_raw(a,b) >= 0
```

This generic support test derives transport from composition. An obstructed channel becomes its
opposite direction in the same cell. This rule makes every oriented interface either transmit or
reflect symmetrically, so transport is a permutation of channels on a fixed Cartesian medium.
Composition is a static substrate here: reflection transfers momentum to that substrate without
an explicit recoil variable. Global **packet** momentum therefore need not be conserved at
reflectors, while packet number remains exact. Treating packet count as energy is a possible
interpretation, not a coupling to the current material potential.

## What might emerge

Geometry can form guide channels and reflective cavities. Composition can join or separate
signal domains. Packet trains can carry timing and information; head-on pairs plus a spectator
offer conditional routing. A useful experiment would build a clock loop and a controlled route
using only geometry, composition, and initial packet states. Gate layouts and universal computation
are **not demonstrated** by the current code. The lattice has cubic anisotropy; the direction
formula introduces material orientation and needs criticism. Fluid pressure, entropy, fracture,
matter/packet work conversion, and arbitrary machinery are absent.

This is deliberately unlike the wave candidate: finite occupation and nonlinear collisions make
exact symbolic signals convenient, while linear amplitude interference is unavailable. It is
also unlike reservoir diffusion: transport retains time-of-flight and directional information.

## Run and compare

Install into a PWC 3.2+ build and turn on `pwc.physics-packets.enabled`, keeping other model
providers off. `Ctrl+F2` injects one external packet into the player's canonical cell. The first
available channel is chosen starting at +x. For direct laboratories,
`perturb(pos, amount)` uses `(amount-1) module 6` as its first channel, then searches cyclically for a
free one. An occupied six-channel cell or the global 256-packet cap refuses further input and
counts it. Input is local single player; clients do not authoritatively seed multiplayer worlds.
The HUD samples the player's current cell and reports the total count; no packet renderer is
included. The root lab can install `spec()` directly.

```sh
pwc instance create physics-packets-lab
pwc --instance physics-packets-lab module add ./modules/physics-packets
pwc --instance physics-packets-lab build --profile dev --print-dir
# In the generated workspace printed by the builder:
cargo test -p pwc_physics_packets --lib -- --nocapture
```

The focused demonstration enumerates all 192 mask/axis cases, verifies that scattering is
involutive and conserves count and momentum, and contrasts a head-on pair with/without a
spectator. This validates the local algebra, not arbitrary circuits or complete world evolution.

## Bounds, saves, and topology

At most 256 packets exist. Hence at most 256 active cells need processing, and the default
384-unit budget permits one synchronous scattering/transport step. A smaller quota pauses the
whole step when necessary. Empty cells consume no dynamical storage. Work is at most six channel
tests per active cell. The cross-fit test costs up to `32*32` pair fits and is deliberately
unoptimized; caching it by configuration pair is a clear experiment after useful behavior is
found. State is saved deterministically, rejects invalid masks, excess counts, and duplicate
positions, and restores transactionally.

The step constructs a separate output map and commits only when all packets were routed. A
duplicate output channel or unreadable source refuses the **entire** transition and counts a
fault; it never merges packets by clipping occupation. A chart crossing is explicitly refused:
the current host resolves cell identity but does not expose the frame rotation needed to carry
oriented channels between planet charts. Test Cartesian areas and chart interiors. Packet number
remains unchanged when geometry is edited between complete steps, but reversibility requires
the same medium when running the inverse.

## Inspirations and original choices

[Margolus, *Physics-like models of computation* (1984)](https://fab.cba.mit.edu/classes/862.22/notes/computation/Margolus-1984.pdf)
and [*Universal Cellular Automata Based on the Collisions of Soft Spheres*](https://arxiv.org/abs/0806.0127)
show that conservative local dynamics can support computation in carefully constructed systems.
This module does not copy their collision rules or inherit their universality results. Its proposed
PWC experiment is the spectator-conditioned head-on axis exchange plus symmetric interfaces
derived directly from existing element fit. Whether those choices yield accessible and useful
player-built machines must be demonstrated, and alternative orientation rules deserve comparison.

Code: AGPL-3.0-or-later. Documentation: CC-BY-SA-4.0. No source from the papers was copied.

## Package management

This source package lives in `pwc-package-manager/modules/physics-packets`. `module.toml` is
canonical; the package manager generates Cargo manifests and builds a content-addressed source
snapshot. It depends only on module API 3.2. Add it by id after `pwc setup`, or use
`pwc --instance <lab> module add ./modules/physics-packets` from the manager checkout.
Use an isolated test instance; fields are singleplayer experiments, not multiplayer visuals.
The source and documentation retain their existing AGPL/CC licences; complete texts are included.
