# Hotbar (`pwc.hotbar`)

Nine slots of held materials plus the bare hand, along the bottom of the screen.

A slot names a stack in the core inventory (the player's held materials, which the game keeps). The
selected slot is what the player *holds*: a left click with it runs the law between the held
material and the targeted block (in PWC a tool is just a block in hand), a right click places one
unit of it. Slot 0, the bare hand, breaks blocks. Newly gathered materials drop into the first
empty slot, and when a tool reaction changes the held unit (a pick wears down, a chisel turns into
something else) the slot follows it to its new configuration. The bar shows each slot's colour,
accent and count, and the name of what is held (the tool name from a namer such as
`pwc.material-names`, or "Bare hand").

Mod id: `hotbar`. Part of `pwc.essentials`.

## Controls

| Input | Action |
|---|---|
| `1`-`9` | Select that slot; pressing the selected slot again returns to the hand |
| `0` | Select the bare hand |
| Mouse wheel | Cycle through the hand and the nine slots (wraps around) |
| Left click | Use the held material as a tool on the targeted block (bare hand: break it) |
| Right click | Place one unit of the held material |

While an inventory panel is open (`pwc.inventory`), the number keys and the wheel belong to that
panel, which uses them to equip materials into slots.

## Settings

None.

## Persisted state

- **World save:** one line under the mod id `hotbar`, payload version 1:
  `v1;sel=<selected>;<key>=<material spec>;…` — the selected slot (0 = hand) and the portable
  spec of every filled slot. Older saves keyed by the display name ("Hotbar") still load.

Placed units are spent from the core inventory, not from the hotbar, so the inventory stays the one record
of what the player holds.

## For mod authors: the shared handles

`register` provides two handles through the `ModRegistrar`, so packages that depend on
`pwc.hotbar` can work with the same bar:

```rust
use pwc_hotbar::{HotbarHandle, ItemUiHandle, HotbarState, SLOTS};

pub fn register(registrar: &mut pwc_mod_api::ModRegistrar) {
    let bar = registrar.get::<HotbarHandle>().expect("pwc.hotbar registered first");
    let ui = registrar.get::<ItemUiHandle>().expect("pwc.hotbar registered first");
    // bar.get() / bar.set(..) / bar.update(|s| s.equip(3, id)); ui.inventory_visible() …
}
```

- `HotbarHandle` — the slots and the selection (`HotbarState`: `slots[i]` is key `i + 1`,
  `selected` is 0 for the hand or 1..=9).
- `ItemUiHandle` — whether an item panel is open (`ItemUiState::inventory_visible`). While it is,
  the hotbar ignores the number keys and the wheel.

Both are cheap `Rc` handles; clones share state. Mod hooks run on the game thread only.

## Dependencies

None besides the PWC mod API (`pwc-api ^2.1`).

## Compatibility

- `tool` is a first-active-wins hook: another mod that reports a tool and
  registers earlier takes precedence over this bar.
- Disabling the hotbar leaves no tool, so a primary action breaks into the inventory. The number
  keys are this mod's actions, so they select nothing while it is off. The inventory is core state,
  so nothing carried is lost.
- In multiplayer the server evaluates tool use and placements; a rejected placement is refunded
  by the core.
- Mod ids, the save key and the payload format are unchanged from the hotbar that shipped inside
  PWC 2.0, so existing worlds keep working.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
