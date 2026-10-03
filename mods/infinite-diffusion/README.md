# InfiniteDiffusion (`pwc.infinite-diffusion`)

PWC's world generator: you start on a 50,000,000-block cube planet whose six faces are themed realms
(mountain ranges, carved valleys and many kinds of country), with caves, abandoned mines and a
deep interior below, in a universe of round worlds, moons and asteroid swarms. The terrain itself is part of the game's deterministic core
(it feeds the multiplayer content fingerprint and must be bit-identical everywhere); this mod
selects it for new worlds and carries its knobs. With the mod disabled, new worlds use the core's
flat world.

Mod id: `diffusion` (display name "InfiniteDiffusion"), in the Essentials group, enabled by
default.

## Controls

None.

## Settings (mods screen)

All knobs apply to **new** worlds.

| Knob | Range | Step | Default | Effect |
|---|---|---|---|---|
| Relief | 25 – 200 % | 25 | 100 % | Mountain height |
| Caves | 0 – 200 % | 25 | 100 % | Cave density |
| Mines | 0 – 200 % | 25 | 100 % | Abandoned-mine density |
| Space | 0 – 200 % | 25 | 100 % | Asteroid-swarm density |
| Variety | 0 – 200 % | 25 | 100 % | How much the land changes from place to place |
| Features | 0 – 200 % | 25 | 100 % | Surface feature density (spires, arches, crystals, …) |
| Structures | 0 – 200 % | 25 | 100 % | Ruin and structure density |
| Deep | 0 – 200 % | 25 | 100 % | Deep-interior cavern and strata density |

## Persisted state

- **World save:** one line under the mod id `diffusion`, payload version 2:
  `v2;relief=<r>,caves=<c>,mines=<m>,space=<s>,variety=<v>,features=<f>,structures=<t>,deep=<d>`
  (a payload without the last four keys leaves them at 100 %). Which generator a saved world uses is recorded in
  the save header, so turning the mod off never changes an existing world.
- **`mods.cfg`:** `diffusion=on|off` and
  `diffusion.state=relief=100,caves=100,mines=100,space=100,variety=100,features=100,structures=100,deep=100`. Stray values snap onto the 25 %
  stepper. A `mods.cfg` from before format version 2 (which recorded `diffusion=off` for everyone)
  does not switch the generator off.
- `WATT_BENCH_WORLDGEN=flat|diffusion` (benchmark runs) pins the generator for that run without
  writing `mods.cfg`.

## Dependencies

None besides the PWC mod API (`pwc-api ^1.0`).

## Compatibility

- `worldgen` is first-enabled-wins: another world-generator package registered earlier takes over
  new worlds while both are enabled.
- In multiplayer the server's generator and settings decide; they are part of the content
  fingerprint a client must match to join.
- The mod id, save key and payloads are unchanged from the mod that shipped inside PWC 2.0, so
  existing worlds and `mods.cfg` files keep working.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
