<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->

# Memory matter

An original fictional simulation experiment inspired by
[rotor-router walks](https://arxiv.org/abs/math/0703453): an impulse changes the path taken by
the next impulse. Every material follows the same routing law. There are no special wire,
gate, counter, or clock types. The rotor is the extra memory, separate from material identity.

Build this directory as a normal native PWC mod through the package manager. It requires API
3.2. Enable its **Memory Matter Physics** world option; it defaults off. Only the first enabled
simulation provider is selected, so disable other physics providers when comparing it. State stays with
the world's model while the mod is switched off; save compatibility is `pwc.physics-rotor`,
law version 1.

In singleplayer, press **Ctrl+F4** to inject one impulse at your current cell. The first accepted
input anchors a closed 7×7×7 laboratory there. Stay within three cells along each coordinate;
later input outside that patch is refused. The HUD shows total and local impulse counts, the
current cell's rotor, turns, and refusals. Initial all-air geometry also carries impulses, so
this experiment does not require a specially named conducting material. Place or remove
ordinary matter to change routing, then inject another impulse and watch how its path differs.
No simulation input is sent by this hook during multiplayer.

## State and law

Each visited site remembers a count `n` and a rotor index `r` in `0..6`. Unvisited sites begin
with `n=r=0`. A complete logical turn gives every site with impulses at the start exactly one
departure attempt. An incoming impulse waits until the next turn. One rule applies to every
configuration `C`, including the empty multiset:

```text
s = sum over occurrences e in C of (e[0] + 3e[1] + 5e[2] + 7e[3])
orientation = s mod 6
reverse = (floor(s/6) + number of occurrences in C) mod 2
port(slot) = (orientation + (reverse ? -slot : slot)) mod 6
```

Ports `0..5` are `+X,-X,+Y,-Y,+Z,-Z`. Starting at `r`, try each of six slots cyclically;
send one impulse through the first available port and retain `r=slot+1 mod 6`. If none is
available, retain both count and rotor. Availability means the neighboring cell exists and
lies inside the finite laboratory, with no chart-seam remapping. The patch boundary thus
redirects the scan to another available face; it never deletes an impulse. This is a routing
rule, not billiard reflection or a momentum conservation claim.

The weighted sum is commutative over the multiset. It uses actual element coordinates and
cardinality, never intern IDs, material names, recipes, or a per-material behavior table.
The weights and port order are deliberately chosen universal law constants; the model is
not claimed to be free of design choices or invariant under translations of resource space.
Modulo routing also changes abruptly when a resource byte wraps from 255 to 0, despite those
coordinates being neighbors on the resource torus; smooth material similarity is not guaranteed.
Editing matter remaps the same stored rotor to its new port order on the next turn. Matter
itself does not move or react in this experiment.

Closed turns conserve total impulses exactly. Player injection is an explicit external source,
not spontaneous generation. The laboratory stores at most 343 visited sites, with at most
65,535 impulses per site. Excess input is refused in full. All departures and arrivals are
proposed from the initial turn state; any capacity failure refuses the whole turn before
changing any site. The `dropped` API counter records refusals, never destroyed impulses.

The work quota counts initially active departure attempts, with at most six fixed port probes
per attempt. If the whole turn exceeds the available quota, the model waits with its state
intact. The default 384-attempt budget covers the maximum 343-site domain. Reducing the quota
below current activity pauses the laboratory; it does not publish a partial update or change
the law's timing. Visited empty sites retain their rotors as memory. With no impulses, the
model sleeps. Saves contain the anchor, sparse site state, and counters; malformed state is
refused before replacing the current model.

## What it might offer, and what is still unknown

Transport writes local memory, suggesting counters, sorting paths, geometry-dependent
circulation, and history-sensitive signal routing. A tiny closed four-cell square fixture
found an eight-turn repeating state with one conserved impulse. Save/reload continued the same
state sequence. That demonstrates one recurrent construction, not a universal computer, a
robust gate, or useful gameplay.

The model conserves impulse count. It does not conserve or even define physical energy or
momentum, and is not automatically time reversible. A rotor is already a designed memory
mechanism; its benefit is a small common rule that players can arrange, not mathematical
realism. Air and material both route, and the finite boundary participates in a discovered
cycle. Real wire confinement, load tolerance, direction bias, and simultaneous arrivals need
further investigation. There is no measured performance claim yet.

The laboratory deliberately supports Cartesian space or the interior of one chart only.
`CellStore::resolve` does not rotate direction channels between chart frames; a remapped source
or destination is unavailable here. It also does not map the patch onto a curved physical
volume or neighboring planets. General topology support must precede larger experiments.

From the repository's Nix development shell, which supplies ALSA and D-Bus build metadata,
run the small conservation, restart, and discovery check:

```sh
pwc instance create physics-rotor-lab
pwc --instance physics-rotor-lab mod add ./mods/physics-rotor
pwc --instance physics-rotor-lab build --profile dev --print-dir
# In the generated workspace printed by the builder:
cargo test -p pwc_physics_rotor --lib -- --nocapture
```

The source uses AGPL-3.0-or-later; this documentation uses CC-BY-SA-4.0. Both full license
texts are included in `LICENSES/`.

## Package management

This source package lives in `pwc-package-manager/mods/physics-rotor`. `mod.toml` is
canonical; the package manager generates Cargo manifests and builds a content-addressed source
snapshot. It depends only on mod API 3.2. Add it by id after `pwc setup`, or use
`pwc --instance <lab> mod add ./mods/physics-rotor` from the manager checkout.
Use an isolated test instance; fields are singleplayer experiments, not multiplayer visuals.
The source and documentation retain their existing AGPL/CC licences; complete texts are included.
