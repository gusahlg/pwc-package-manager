# Changelog — pwc.sounds

## 1.1.1 — modules, not mods

- The package says module everywhere: `module.toml`, `kind = "module"`, `pwc_module_api` and the `Module*` names, `pwc-api ^3.4`. Nothing else changes; the id is the same.

## 1.1.0

- No description or group of its own: the package manifest is what menus show. Mods are no longer
  switched in the game; the build decides what is in.
- Requires `pwc-api ^3.0`.

## 1.0.0

- Mod id `sounds`, in the Essentials group, enabled by default. Requires `pwc-api ^2.1`.
- Plays the same cues the game used to choose itself: class cue, then `*_default`, plus `swing`,
  `menu_click` and `voicetest`.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
