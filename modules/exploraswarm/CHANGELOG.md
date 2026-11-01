<!-- SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Changelog — pwc.exploraswarm

## 0.1.1 — modules, not mods

- The package says module everywhere: `module.toml`, `kind = "module"`, `pwc_module_api` and the `Module*` names, `pwc-api ^3.4`. Nothing else changes; the id is the same.

## 0.1.0

- Headless neural-network players join a server and explore it with a MAP-Elites archive.
- Random, Gaussian and IsoLine emitters, with a curiosity-biased parent pick and a two-evaluation noise check.
- `explorer` and `builder` rule sets, a discovery log, checkpoints, and the `swarm` command.
