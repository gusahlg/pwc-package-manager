# Changelog — pwc.infinite-diffusion

## 1.1.0

- Relief, caves, mines and space are options in the game's options registry (the World page of any
  settings screen; `settings.cfg` keys `pwc.infinite-diffusion.<key>`), not knobs on a mods screen.
  They apply to new worlds. The per-world save line is gone: a saved world's generator settings
  live in its header. No description or group of its own.
- The values an older game kept in `mods.cfg` carry over once (legacy keys).
- Requires `pwc-api ^3.0`.

## 1.0.1

- Requires `pwc-api ^2.0`. Behaviour unchanged.

## 1.0.0 — first packaged release, moved out of the PWC source tree

- Mod id `diffusion`; save line (payload v2) and knob payload unchanged.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
