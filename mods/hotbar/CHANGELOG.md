# Changelog — pwc.hotbar

## 2.1.0 (unreleased)

- No description or group of its own: the package manifest is what menus show. Mods are no longer
  switched in the game; the build decides what is in. Save format unchanged.

## 2.0.1

- Requires `pwc-api ^2.1`. Each action names `held: false`. Behaviour unchanged.

## 2.0.0

- Requires `pwc-api ^2.0`. Declares `hotbar.hand`, `hotbar.slot1`…`hotbar.slot9`, `hotbar.next` and `hotbar.prev` on the same default keys (0, 1–9, wheel).
- Answers `Mod::tool` and shows the tool note above the bar.
- Save line unchanged: `v1;sel=…`.

## 1.0.0 — first packaged release, moved out of the PWC source tree

- Mod id `hotbar`; save line `hotbar` (payload v1) unchanged.
- New public API for dependent packages: `HotbarHandle`, `ItemUiHandle`, `HotbarState`,
  `ItemUiState`, `SLOTS`, provided through the registrar.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
