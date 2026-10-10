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

Mod id: `material_names`. Part of `pwc.essentials`.

## Controls

None.

## Settings (the Interface page)

An option in the game's options registry, listed on the Interface page of any settings screen.

| Option | Values | Default | Effect |
|---|---|---|---|
| Material Names | Mineral / Arcane | Mineral | Switches the vocabulary of suffixes and qualifiers (mineral-like "-ite", "Banded" or fantasy-like "-ael", "Runic") |

Switching the style renames everything at once (the namer's revision changes).

## Persisted state

- **World save:** nothing. Names are never saved; they are recomputed from the configuration.
- **`settings.cfg`:** `pwc.material-names.style=mineral|arcane`.

## Dependencies

None besides the PWC mod API (`pwc-api ^2.0`).

## Compatibility

- `namer` is first-active-wins: another naming package registered earlier replaces these names
  while both are installed. Without one, the game describes materials by their readings.
- Names are presentation only: they never reach the law, world generation, saves or the network.
- The mod id is unchanged from the mod that shipped inside PWC 2.0. The style saved in `mods.cfg`
  is not carried over: it starts at Mineral.

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
