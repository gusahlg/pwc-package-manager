# Inventory (`pwc.inventory`)

The core inventory made visible, and the way to equip what you hold.

PWC's core inventory is a list of configurations. Counts of one configuration add together. There
is no grid, and the list is unreachable without a mod. This package adds a panel listing every
material you hold, with its count, its number of elements, its colour swatch and the hotbar slot
it sits in. Choose a material and put it on the hotbar (`pwc.hotbar`).

When a break overflows the inventory (units are destroyed because it is full), the panel's header —
or, while the panel is closed, a line at the top of the screen — warns
"Inventory full - elements lost!" for 2.5 seconds after the last overflowing break.

Mod id: `inventory` (display name "Inventory"), in the Essentials group, enabled by default.

## Controls

| Input | Action |
|---|---|
| `I` | Open / close the panel |
| `↑` / `↓` | Move the cursor |
| `1`-`9` | Equip the material under the cursor into that hotbar slot (and select it) |
| `Enter` | Equip into the selected slot, or the first free slot while the hand is selected |
| `Esc` | Close the panel (before Escape would leave the world) |

While the panel is open it owns the number keys and the wheel; the hotbar gets them back when it
closes.

## Settings

None.

## Persisted state

- **World save:** nothing. The counts live on the player (the core inventory), which the game saves
  itself; the hotbar saves its slots.
- **`mods.cfg`:** only `inventory=on|off`.

## Dependencies

- The PWC mod API (`pwc-api ^2.0`).
- `pwc.hotbar ^2.0` — the inventory equips through the hotbar's shared `HotbarHandle` and tells it
  that the panel is open through the shared `ItemUiHandle`. The dependency guarantees that the
  hotbar registers (and provides the handles) first; registering the inventory without it panics
  with a message naming the missing handle.

## Compatibility

- Disabling the inventory makes the list inaccessible again but keeps every unit on the player.
- `close_overlay` (Escape) is first-enabled-wins: an open inventory panel consumes Escape.
- Mod id and `mods.cfg` key are unchanged from the inventory that shipped inside PWC 2.0.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
