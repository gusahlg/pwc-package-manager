<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Managed physics laboratory

This library supplies headless instruments for fifteen independent simulation packages. It
registers no gameplay hooks. Its declared dependencies make the manager compile the real module
packages into one content-addressed build; it does not ship a Cargo manifest or another launcher.
Installing the library also installs its physics dependencies. Selective transfer starts enabled;
all experimental providers default off. Gameplay still uses one selected authority model.

## An isolated managed instance

From the package-manager checkout, use an isolated manager home for the whole command sequence:

```sh
export PWC_HOME=/tmp/pwc-physics-managed
pwc setup --pwc ../project_watt_cubed --repository ./mods
pwc instance create physics-lab
pwc --instance physics-lab module add ./modules/physics-lab
pwc --instance physics-lab build --profile dev --print-dir
```

The last command prints the generated workspace after building its runtime. In that directory,
with the same development shell's native libraries available:

```sh
cargo test -p pwc_physics_lab --lib -- --nocapture
PWC_PHYSICS_MODEL=dipole,frames cargo test -p pwc_physics_lab \
  short_all_model_demonstrations_and_full_state_round_trips -- --nocapture
cargo test -p pwc_physics_loops --lib -- --nocapture
```

`PWC_PHYSICS_MODEL` optionally filters headless demonstrations by one or more comma-separated mode names; modes are `off`, `selective`,
`wave`, `packets`, `reservoir`, `rotor`, `fuel`, `cyclic`, `loops`, `spin`, `symplectic`, `bonds`,
`cooperative`, `permutation`, `dipole`, and `frames`. This variable is used only by the demonstration test; it is
not a gameplay selector. Tests use the real factories and API harness. JSON readouts report model
id, requested ticks, accepted turns, operations, pending work, refusals, matter, four instrument
values, and reserved-state string bytes. Units differ by model: counters are not equal-cost
performance measurements. Every run checks complete model-record save/load equality.

For visible gameplay instruments, add the normal experience to this same disposable instance:

```sh
pwc --instance physics-lab module add pwc.essentials
pwc --instance physics-lab run --profile dev
```

In Settings, disable all other physics providers and enable the one you want. The first enabled
provider in registration order wins; installing packages does not compose their laws. A simpler
one-model instance can omit this library and install only Essentials plus the chosen physics module.
The new models share Ctrl+F5 because only the selected model consumes its input; earlier models
use the keys in their own READMEs. Use separate worlds/instances for physical comparisons.

## Interpreting a demonstration

The fixtures are deliberately different inputs, not equal-resource trials. Old selective and
reservoir modes share the reference two-material pair; cooperative uses a genuine four-occurrence
single-transfer local trap; permutation starts with one movable occurrence. Wave and torus maps
receive opposite impulses, packets receive a head-on pair plus spectator, bonds receive imposed
displacement, spin receives local reservoir energy, and other models receive their explicit
laboratory seed. Dipole receives an adjacent neutral pair; frames receives a single
noncommuting disturbance in an otherwise uniform prepared medium. Those seeds are external preparation, not spontaneous material creation.

All dynamic fields are instance state, not extra `BlockId`s. Most models use bounded laboratory
domains. Mathematical invariants apply under their stated assumptions; they do not establish
scalable worlds, familiar electricity, fluid mechanics, useful computation, player forces,
equilibrium thermodynamics, or enjoyable technology. Field readouts and experimental inputs are
singleplayer only; networked field replication/port transforms remain future work.

The library touches no files, starts no processes and makes no network requests during gameplay.
Its tests print instruments; `PWC_PHYSICS_MODEL` is read only by tests. Its own source is licensed
Apache-2.0 or MIT, at your option. Dependencies retain their individual licences; the combined
game remains under the game's AGPL. The five migrated packages retain AGPL code and CC-BY-SA
documentation. Full licence texts for this library are in `LICENSES/`.
