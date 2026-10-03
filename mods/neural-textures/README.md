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

Mod id: `neural_textures` (display name "Neural textures"), in the Essentials group, enabled by
default.

## Controls

None.

## Settings (mods screen)

| Knob | Range | Default | Effect |
|---|---|---|---|
| Detail | 0.1 – 2.0 (steps of 0.1) | 1.0 | Spatial frequency of the pattern |
| Contrast | 0.1 – 2.0 (steps of 0.1) | 1.0 | Spread of the palette around the material's colour |

Changing a knob repaints every texture (the appearance revision changes).

## Persisted state

- **World save:** nothing.
- **`mods.cfg`:** `neural_textures=on|off` and `neural_textures.state=detail=<d>,contrast=<c>`
  (one decimal each; out-of-range values clamp, unparseable ones are ignored).

## Dependencies

None besides the PWC mod API (`pwc-api ^1.0`).

## Compatibility

- `appearance` is first-enabled-wins: another appearance package registered earlier replaces these
  textures while both are enabled. Disabled, blocks use the core's flat colours.
- Presentation only: textures never feed back into the law, world generation, saves or the network.
- The game paints a texture once per configuration (and again only when a knob changes), never per
  frame.
- The mod id and knob payload are unchanged from the mod that shipped inside PWC 2.0.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
