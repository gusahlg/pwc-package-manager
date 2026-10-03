# Visuals (`pwc.visuals`)

PWC's core renderer is sunlight-only. This package restores the full look with three mods, one per
render group:

| Mod (id) | Render group | What it turns on |
|---|---|---|
| Atmosphere (`atmosphere`) | Atmosphere | Sky, clouds, weather, stars, day/night, and fog |
| Post (`post`) | Post | Bloom, god rays, TAA, exposure, vignette, and variable-rate shading |
| Lighting (`lighting`) | Lighting | Shadows, ambient fill, and block light (sunlight stays in the core) |

All three are in the Essentials group and enabled by default. Disable one and its settings lanes
are forced off; the settings menu and `/gfx` mark them "(off: <mod> mod)". Disable all three for
the core look.

## Controls

None. The individual lanes (clouds, bloom, shadows, …) are game settings (the Settings screen,
or `/gfx`); these mods decide whether each group may be on at all.

## Settings

None on the mods screen.

## Persisted state

- **World save:** nothing.
- **`mods.cfg`:** `atmosphere=on|off`, `post=on|off`, `lighting=on|off`.
- `WATT_BENCH_VISUALS=core` (benchmark runs) switches all three off for that run without writing
  `mods.cfg`.

## Dependencies

None besides the PWC mod API (`pwc-api ^1.0`).

## Compatibility

- `visual_group` composes: the game ORs every enabled mod's group into its render mask, so another
  package that owns the same group keeps it on even with this package's mod disabled.
- Presentation only: nothing here affects the world, saves or the network.
- Mod ids are unchanged from the visual mods that shipped inside PWC 2.0.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
