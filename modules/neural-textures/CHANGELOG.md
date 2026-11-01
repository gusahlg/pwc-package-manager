# Changelog — pwc.neural-textures

## 1.1.1 — modules, not mods

- The package says module everywhere: `module.toml`, `kind = "module"`, `pwc_module_api` and the `Module*` names, `pwc-api ^3.4`. Nothing else changes; the id is the same.

## 1.1.0

- Detail and contrast are options in the game's options registry ("Texture Detail" and "Texture
  Contrast" on the Video page; `pwc.neural-textures.<key>`), not knobs on a mods screen. No
  description or group of its own.
- The values an older game kept in `mods.cfg` carry over once (legacy keys).
- Requires `pwc-api ^3.0`.

## 1.0.1

- Requires `pwc-api ^2.0`. Behaviour unchanged.

## 1.0.0 — first packaged release, moved out of the PWC source tree

- Mod id `neural_textures`; knob payload unchanged.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
