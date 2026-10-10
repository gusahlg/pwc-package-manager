# InfiniteDiffusion (`pwc.infinite-diffusion`)

PWC's world generator: mountain ranges and carved valleys on the surface, caves and abandoned
mines below, planets in space above. The terrain itself is part of the game's deterministic core
(it feeds the multiplayer content fingerprint and must be bit-identical everywhere); this mod
selects it for new worlds and declares its options. A build without the mod makes the core's
flat world.

Mod id: `diffusion`. Part of `pwc.essentials`.

## Controls

None.

## Settings (the World page)

Options in the game's options registry, listed on the World page of any settings screen. They
apply to **new** worlds.

| Option | Range | Step | Default | Effect |
|---|---|---|---|---|
| Terrain Relief | 25 – 200 % | 25 | 100 % | Mountain height |
| Caves | 0 – 200 % | 25 | 100 % | Cave density |
| Mines | 0 – 200 % | 25 | 100 % | Abandoned-mine density |
| Space | 0 – 200 % | 25 | 100 % | Planet and asteroid density |

## Persisted state

- **`settings.cfg`:** `pwc.infinite-diffusion.relief=`, `.caves=`, `.mines=`, `.space=` (whole
  percents). Stray values snap onto the 25 % stepper.
- **World save:** nothing. Which generator and settings a saved world uses are recorded in the
  save header, so neither the options nor this package change an existing world. A save line from
  an older version (`diffusion`, `v2;relief=…`) is ignored.
- `WATT_BENCH_WORLDGEN=flat|diffusion` (benchmark runs) pins the generator for that run.

## Dependencies

None besides the PWC mod API (`pwc-api ^3.0`).

## Compatibility

- `worldgen` is first-active-wins: another world-generator package registered earlier takes over
  new worlds while both are installed.
- In multiplayer the server's generator and settings decide; they are part of the content
  fingerprint a client must match to join.
- The mod id is unchanged from the mod that shipped inside PWC 2.0, so existing worlds keep
  working. The knob values an older game saved in `mods.cfg` are read once, the first time this
  game loads its settings.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
