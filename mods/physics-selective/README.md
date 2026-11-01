<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->
# Selective transfer, extracted

This is the original PWC contact model as a native optional mod, including its deterministic
arrival-ordered queue, dormant contacts, finite contact budget, tool law and persistent queue.
The material crate still supplies static configuration/observation and palette-search math;
that utility does not activate world or tool reactions without this provider.

Install with `pwc mod add ./mods/physics-selective`. The `pwc.physics-selective.enabled` option
pauses it. The first enabled simulation provider in registration order wins; turn competing
providers off when comparing models. Native registration also works through a `GameBuild`.

The formula is unchanged: an occurrence transfer/swap is accepted when the internal fit gain
exceeds the original size-dependent quantum threshold. World commits now admit both products
atomically, and cell reads resolve chart seam identities. These are storage/authority corrections,
not an additional chemistry rule.

A closed finite system strictly increases a bounded compatibility score, so it eventually rests.
It cannot supply persistent clocks or carry signals through stationary matter by itself. That
limitation motivates the independent wave, packet, reservoir and rotor experiments.

State is retained when disabled/switched away and saved in the core's opaque simulation records.
Old pending-contact records are migrated only into this model. New model versions must use a
new compatibility version. No model changes the static generated palette in this exploration.

## Package management

This source package lives in `pwc-package-manager/mods/physics-selective`. `mod.toml` is
canonical; the package manager generates Cargo manifests and builds a content-addressed source
snapshot. It depends only on mod API 3.2. Add it by id after `pwc setup`, or use
`pwc --instance <lab> mod add ./mods/physics-selective` from the manager checkout.
Use an isolated test instance; fields are singleplayer experiments, not multiplayer visuals.
The source and documentation retain their existing AGPL/CC licences; complete texts are included.
