# Changelog — pwc.visuals

## 1.1.1 — modules, not mods

- The package says module everywhere: `module.toml`, `kind = "module"`, `pwc_module_api` and the `Module*` names, `pwc-api ^3.4`. Nothing else changes; the id is the same.

## 1.1.0

- No description or group of its own. Installed means the three render groups are allowed; a
  suspended package (a refusing server, `WATT_BENCH_VISUALS=core`) strips them, and settings screens
  mark the lanes "(unavailable in this build)" instead of naming a mod.
- Requires `pwc-api ^3.0`.

## 1.0.1

- Requires `pwc-api ^2.0`. Behaviour unchanged.

## 1.0.0 — first packaged release, moved out of the PWC source tree

- Mod ids `atmosphere`, `post`, `lighting`; behaviour unchanged.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
