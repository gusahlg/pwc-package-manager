# Changelog — pwc.game-ui

## 2.0.0 (unreleased)

- The information HUD the core used to draw: the reticle, "Loading world…"/"Loading terrain…", the
  coordinates (with the cruise speed), the frame rate, the player count and ping on a server, and
  the "connection interrupted" banner, with the same layout, text and HUD modes. The core draws no
  HUD text any more.
- Requires `pwc-api ^2.2` (`HudFacts`). The facing indicator is unchanged and hides with the HUD off.
- No description or group of its own: the package manifest is what menus show. Mods are no longer
  switched in the game; the build decides what is in.


## 1.0.1

- Requires `pwc-api ^2.0`. Behaviour unchanged.

## 1.0.0 — first release

- The facing indicator: an axis gizmo in the top-right corner, under the minimap, the facing in words
  (`facing -Z +Y`) and the view direction as a vector.
- Mod id `game_ui`, in the Essentials group, enabled by default. Requires `pwc-api ^1.1`.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
