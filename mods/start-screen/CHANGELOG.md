# Changelog — pwc.start-screen

## 2.0.0

- The root screen of the game's screen host (`Mod::root_screen`) instead of a start screen beside a
  core fallback. It asks the core with `AppRequest`s and draws into the core's `UiElement`s with
  the `pwc.ui-kit` look (new dependency `pwc.ui-kit ^1.0`).
- Mods and Settings are no longer fixed rows: the main menu lists the entries other packages offer
  for it (by order), and opens them by id.
- Draws the waiting page ("Connecting…" / "Loading…", "Cancel") the game used to draw itself.
- No description or group of its own: the package manifest is what menus show. A server can
  suspend the package for a session; mods are no longer switched in the game.
- Requires `pwc-api ^3.0`.

## 1.1.1

- Requires `pwc-api ^2.0`. Behaviour unchanged.

## 1.0.0 — first packaged release, moved out of the PWC source tree

- Mod id `start`; behaviour, pages and texts unchanged.
- Parts of the main page, the Worlds page and the host/join form were reimplemented independently
  so the whole package can be licensed `Apache-2.0 OR MIT`.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
