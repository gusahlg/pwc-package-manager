# Visuals (`pwc.visuals`)

PWC's core renderer is sunlight-only. This package restores the full look with three mods, one per
render group:

| Mod (id) | Render group | What it turns on |
|---|---|---|
| Atmosphere (`atmosphere`) | Atmosphere | Sky, clouds, weather, stars, day/night, and fog |
| Post (`post`) | Post | Bloom, god rays, TAA, exposure, vignette, and variable-rate shading |
| Lighting (`lighting`) | Lighting | Shadows, ambient fill, and block light (sunlight stays in the core) |

Part of `pwc.essentials`. A build without this package has the core look: the lanes of the three
groups are forced off, and settings screens and `/gfx` mark them "(unavailable in this build)".

## Controls

None. The individual lanes (clouds, bloom, shadows, …) are game settings (the Settings screen,
or `/gfx`); these mods decide whether each group may be on at all.

## Settings

None.

## Persisted state

Nothing. `WATT_BENCH_VISUALS=core` (benchmark runs) suspends the package for that run, and a server
that refuses it suspends it for the session; neither is saved.

## Dependencies

None besides the PWC mod API (`pwc-api ^2.0`).

## Compatibility

- `visual_group` composes: the game ORs every active mod's group into its render mask, so another
  package that provides the same group keeps it on even while this one is suspended.
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
