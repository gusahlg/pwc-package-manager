# Changelog — pwc.inventory

## 2.1.0

- Requires `pwc-api ^2.2`. `hud` takes the core's `HudFacts`: the panel and the warning hide with
  the HUD off (the core now asks every mod in every HUD mode). `inventory.toggle` names
  `immediate: false`.

## 2.0.1

- Requires `pwc-api ^2.1`. `inventory.toggle` names `held: false`. Behaviour unchanged.

## 2.0.0

- Requires `pwc-api ^2.0` and `pwc.hotbar ^2.0`. Opens on `inventory.toggle` (`I`).
- Equips through the hotbar's slot actions while the panel is open.
- The panel lists the core inventory. Counts of one configuration add together.

## 1.0.0 — first packaged release, moved out of the PWC source tree

- Mod id `inventory`; behaviour unchanged. The panel header is reworded ("Inventory · N/M held")
  and still shows the held total against the capacity.
- Parts of the HUD and warning code were reimplemented independently so the whole package can be
  licensed `Apache-2.0 OR MIT`.
- Depends on `pwc.hotbar` and equips through its shared handles. Registration now follows dependency
  order, so the hotbar's hooks run before the inventory's (the inventory used to be installed
  first).
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
