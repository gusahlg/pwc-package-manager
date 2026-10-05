# Material names (`pwc.material-names`)

Every material gets a name a person can remember, and its tool a name to match ("Velorine",
"Banded Keshan Olivite", "Velorine Pick").

The words come from a small *language model*: an order-3 character Markov chain (with back-off to
orders 2 and 1) trained once, the first time a name is needed, on a few hundred real mineral, rock
and element names. It speaks in their phonology — "-ite", "-ine", clusters like "rh", "ph",
"ch" — but its roots are new words. Its randomness is a deterministic generator seeded from the
configuration, so every player sees the same names, and the names mean something:

- the **root** comes from the most abundant element (its coordinates coarsened, so near-twins share
  it): a material and everything a reaction makes from it share a family name;
- the **suffix** says how it reads — clear, glowing, hard, firm or soft;
- a **qualifier** names a significant second element ("Keshan …"), a busy mixture ("Banded",
  "Veined") or a mixture that wants to come apart ("Brittle");
- the **tool** name adds a noun by mass and look: shard, chisel or awl, pick, maul or hammer,
  sledge; lens or lantern for clear or glowing tools.

Mod id: `material_names` (display name "Material names"), in the Essentials group, enabled by
default.

## Controls

None.

## Settings (mods screen)

| Knob | Values | Default | Effect |
|---|---|---|---|
| Style | Mineral / Arcane | Mineral | Switches the vocabulary of suffixes and qualifiers (mineral-like "-ite", "Banded" or fantasy-like "-ael", "Runic") |

Switching the style renames everything at once (the namer's revision changes).

## Persisted state

- **World save:** nothing. Names are never saved; they are recomputed from the configuration.
- **`mods.cfg`:** `material_names=on|off` and `material_names.state=style=mineral|arcane`.

## Dependencies

None besides the PWC mod API (`pwc-api ^2.0`).

## Compatibility

- `namer` is first-enabled-wins: another naming package registered earlier replaces these names
  while both are enabled. Disabled, the game describes materials by their readings.
- Names are presentation only: they never reach the law, world generation, saves or the network.
- The mod id and knob payload are unchanged from the mod that shipped inside PWC 2.0.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

The training words are public scientific vocabulary (mineral, rock and element names).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
