# Changelog — pwc.essentials

## 2.0.0 (unreleased)

- Adds the `pwc.chat-commands` bundle (`pwc.chat` and `pwc.commands`): the chat and its commands
  left the core. The chat opens on `§`.
- `pwc.game-ui ^2.0` (it draws the reticle and the readouts the core used to), `pwc.hotbar ^2.1`,
  `pwc.inventory ^2.1` and `pwc.proximity-chat ^1.1`, all on mod API 2.2.
- The menus are packages: `pwc.start-screen ^2.0` (the root screen), and the new `pwc.settings-menu`,
  `pwc.mod-menu` and `pwc.pause-menu` (`^1.0`), which draw with the new library `pwc.ui-kit`.
  `pwc.menus` is retired: its theme is the UI kit's default look.
- Depends on `pwc.game-ui ^2.0`, `pwc.hotbar` and `pwc.inventory` `^2.1`, and the other members at
  `^1.1` (options in the game's registry). Mods are no longer switched or grouped in the game: the
  bundle is how the first-party packages are installed together.


## 1.3.0

- Adds `pwc.sounds` and `pwc.proximity-chat` (`^1.0`), both enabled by default. They need a PWC
  whose mod API is 2.1 or newer. The other members stay on their current constraints.

## 1.2.0

- Depends on `pwc.hotbar ^2.0` and `pwc.inventory ^2.0` (mod API 2.0). The other members stay `^1.0`.

## 1.1.0

- Adds `pwc.game-ui` (`^1.0`): in-world HUD pieces, starting with the facing indicator. It needs a
  PWC whose mod API is 1.1 or newer.

## 1.0.0 — first packaged release, moved out of the PWC source tree

- Bundles the eight first-party packages that used to be compiled into PWC as its built-in mods.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
