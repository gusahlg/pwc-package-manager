# Neural textures (`pwc.neural-textures`)

Every material paints its own 32×32 texture with a CPPN (a compositional pattern-producing
network — the small generative neural net behind a long line of pattern art). There is no training
data and no table of looks: the network is *grown from the material's configuration*. Each distinct
element contributes one first-layer neuron whose weights, frequency and activation are hashed from
its four coordinates, and whose output gain follows its multiplicity; the second layer is wired from
the whole configuration's digest. A block that gains a constituent grows a neuron, configurations
that share elements share structure, and every distinct configuration gets a texture of its own.

The palette comes from the law's presentation (element colours, the impurity accent); hardness draws
crystalline veins, weak cohesion draws grain, glow lights the pattern's crests, clarity sets alpha.
Textures tile seamlessly and are deterministic: every machine paints the same pixels.

Mod id: `neural_textures`. Part of `pwc.essentials`.

## Controls

None.

## Settings (the Video page)

Options in the game's options registry, listed on the Video page of any settings screen.

| Option | Range | Default | Effect |
|---|---|---|---|
| Texture Detail | 0.1 – 2.0 (steps of 0.1) | 1.0 | Spatial frequency of the pattern |
| Texture Contrast | 0.1 – 2.0 (steps of 0.1) | 1.0 | Spread of the palette around the material's colour |

Changing one repaints every texture (the appearance revision changes).

## Persisted state

- **World save:** nothing.
- **`settings.cfg`:** `pwc.neural-textures.detail=` and `pwc.neural-textures.contrast=` (one
  decimal each; out-of-range values clamp, unparseable ones keep the default).

## Dependencies

None besides the PWC mod API (`pwc-api ^3.0`).

## Compatibility

- `appearance` is first-active-wins: another appearance package registered earlier replaces these
  textures while both are installed. Without one, blocks use the core's flat colours.
- Presentation only: textures never feed back into the law, world generation, saves or the network.
- The game paints a texture once per configuration (and again only when an option changes), never
  per frame.
- The mod id is unchanged from the mod that shipped inside PWC 2.0. The values an older game saved
  in `mods.cfg` are read once, the first time this game loads its settings.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
