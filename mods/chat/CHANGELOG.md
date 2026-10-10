# Changelog — pwc.chat

## 1.0.0 — first release, moved out of the PWC core

- The chat line, now on `§` (the physical key left of `1`) instead of `T`; `Esc` or `§` on an
  empty line closes it. History, editing keys and Tab completion as before.
- A sent line goes to the mods' line handlers first (`ChatHandle::add_handler`), then out as local
  chat, or global with a leading `!`. Single player echoes it.
- The scrollback shows chat, joins, leaves and the game's notices with the old colours.
- Mod id `chat`. Requires `pwc-api ^2.2` (the frame hook, text capture, the message stream and the
  HUD facts) and `pwc.ui-kit ^1.0` (the text widgets).
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
