# The search for a fundamental law: brief for the candidate packages

<!-- Working brief for the candidate-law packages (2026-10-13). The owner's question, verbatim in
spirit: what is the simplest, most coherent set of fundamental quantities and interaction laws
that supports a rich, deeply emergent, efficient and enjoyable world? -->

## Why the current law cannot be the whole universe

Selective transfer v1 moves one element occurrence between face-adjacent blocks when it raises
the summed internal fit by more than 1/32. That fit sum is a bounded potential which every
operation strictly increases. A bounded, strictly increasing potential is a **Lyapunov
function**: every closed region must stop. The law is gradient descent, so it is Aristotle's
physics: things move toward their natural place and halt. It can therefore never make, on its
own:

- a persistent oscillation (a clock), since a clock needs a cycle and descent has none;
- a signal that passes through stationary matter and leaves it ready for the next one;
- transport of anything except the elements themselves;
- amplification (gain), since there is no stored energy for a small cause to release.

A law that can do these needs one of two escapes:

1. **Conservation (Newton/Hamilton).** Replace descent by dynamics that conserve a quantity
   ("energy") and are deterministic and ideally reversible. Nothing halts by itself. Heat,
   waves, oscillation and transport come for free. Dissipation appears only *statistically*,
   as order spreading into many degrees of freedom. God does not play dice, and determinism
   still gives the arrow of time as coarse-grained behaviour.
2. **A drive (Prigogine/Belousov).** Keep dissipation but feed the system a source of free energy.
   An excitable medium then sustains waves, spirals and clocks indefinitely. This is how life
   and the Belousov–Zhabotinsky reaction escape equilibrium.

Each candidate below takes one of these escapes, or a different representation altogether.
None of them is assumed correct. Each must earn its place through measurements.

## The rules every candidate follows

- **Real matter in, constants out.** A candidate runs on a `pwc_law_core::Matter` box of the
  game's own configurations. Every constant it uses (stiffness, threshold, mass, conductivity,
  permittivity, period) is *derived* from the fit readings `Matter` offers: `Reading` (cohesion,
  internal fit, light/glow/grip probe responses, occurrence count) and `Matter::bond(a, b)`, the
  mean cross-face fit. No material is named, no table of material behaviours exists, and no
  per-material exception is allowed. Document every derivation formula in the README.
- **Deterministic integer law.** `step` uses integer arithmetic only, so every peer computes the
  same bytes. It may not use randomness, hashing order, wall-clock time or floating point (floats
  are fine in reports and benches). Update order is fixed and documented; synchronous (double
  buffered) is preferred when it is the honest choice.
- **One tick = `Candidate::step`.** `poke` is how a player acts on the law: the gameplay verb.
- **State honesty.** `state_bytes_per_cell` and `active` must be truthful. If a law must touch
  every cell every tick, say so.
- **Constraints.** The law code depends only on `pwc_law_core` and `pwc_mod_api::material`
  (`use pwc_mod_api::material;` at the crate root, then `crate::material::...`). There are no
  other dependencies, no `unsafe` and no `build.rs`. Use Rust edition 2024.

## What each package delivers

`mods/law-<id>/` is a pwc **library** package. Copy the shape of `mods/law-core`: `mod.toml`
(kind `library`, `pwc-api = "^3.1"`, version `0.1.0`, `[dependencies] "pwc.law-core" = "^0.1"`),
`CHANGELOG.md`, `LICENSES/` and `assets/icon.svg` (copy law-core's), `README.md`, and `src/`.

- `src/lib.rs`: the law and `pub const ENTRY: pwc_law_core::Entry`, whose `build` returns the
  candidate at rest on the given matter.
- `src/tests.rs` (`#[cfg(test)]`): the **experiments**. Each experiment is a test that builds
  matter with `pwc_law_core::search` (never hard-coded element bytes unless a search proved them),
  runs the law, prints an ASCII view (`pwc_law_core::show`) under `--nocapture`, and **asserts
  the measured phenomenon** (a period, a speed, a conserved total, an exact reversal, a truth
  table). A phenomenon that the law cannot produce gets a test that demonstrates the failure, or
  an honest README entry when a test cannot express it.
- `physics-lab/laws/<id>/examples/bench.rs`: a release-mode benchmark. Report ns per cell-tick
  and cells per second on a 64³ box that is about half filled with a few real materials (a
  terrain-like slab), both at rest and while busy (after pokes). Report bytes of state per cell.
- `README.md` follows the brief's questions, in this order, with numbers from the experiments:
  1. **Inspiration**: the Soviet mathematics or physics (with authors and years) and the science
     fiction that shaped the law, and what was taken from each.
  2. **What the universe remembers**: the state per cell and per face.
  3. **The law**: exact rules and every constant's derivation from fit.
  4. **What emerges**: each demonstrated phenomenon with its measurement and test name.
  5. **What it cannot do** (or does badly), proved where possible.
  6. **Cost**: the bench numbers, sparsity, memory, and what a world-scale host would need
     (activity tracking, chunking, server authority, save state, network deltas).
  7. **Gameplay**: what a player would discover, build and enjoy; the `poke` verb; what would be
     confusing or frustrating.
  8. **Verdict**: a scorecard of the brief's seven criteria (emergence, generality, expressive
     power, elegance, efficiency, enjoyment, extensibility), 1–5 each, with one line of
     justification per score, plus the strongest objection to the law.

Run with `CARGO_TARGET_DIR=<your scratch dir>` from `physics-lab/`:
`cargo test --release -p pwc_law_<id> -- --nocapture` and
`cargo run --release -p pwc_law_<id> --example bench`.

## The candidates

The ids are fixed, because the workbench and the lab already name them.

### `demon` — the microcanonical law (Landau, Zeldovich, Creutz)

*Escape 1, applied to the existing chemistry.* Keep configurations and selective transfer's
moves, but make the universe keep an **energy ledger**. Each cell also holds `q`, its heat (an
unsigned count of quanta). The total `E = Σ q − Σ binding(cell)` is conserved **exactly**, where
`binding(c) = floor(internal_fit(c) / UNIT)` is a function of the configuration alone. A move is
allowed only if its heat can pay for it:

- an exothermic move (binding rises by Δ) *releases* Δ quanta into the two cells;
- an endothermic move (binding falls) *consumes* quanta from the two cells and needs enough heat;
- **activation**: pulling an occurrence out of its block first costs its current holding
  (`holding()[i] / UNIT`, when positive), which is returned on arrival. Only contacts with that
  much heat can try the move. This makes fuel metastable: it is energetically favourable, but it
  is kinetically locked until heated. That is Zeldovich's flame physics.

Heat moves across faces by exact integer conduction, with a conductance derived from the bond
(compatible neighbours conduct, and void and incompatible faces insulate). Conduction must
conserve Σq exactly, for example with antisymmetric pairwise fluxes computed from the old state.
Choose and justify which move a contact makes, deterministically. The question is whether
**temperature-dependent chemistry** emerges: fire fronts, smelting (heat drives a separation that
cold forbids), thermal runaway and explosions, insulation, and a relaxation oscillator built from
a heat-gated reaction. `poke` adds heat (and must keep the ledger: credit an external-work entry).
Experiments must include: an exact energy conservation test over thousands of ticks in a closed
box; a combustion front with a measured speed; a reaction that occurs only above a temperature;
and an honest attempt at an oscillator or a logic gate.

### `belousov` — the excitable medium (Belousov, Zhabotinsky, Krinsky, Gelfand–Tsetlin)

*Escape 2.* Each cell holds a phase `φ ∈ Z/P`. 0 is rest, `1..=X` is excited and the rest is
refractory. A resting cell fires when the weighted sum of its excited neighbours reaches its
threshold. A firing or refractory cell advances `φ+1 mod P` every tick. The weight of a
neighbour is the **bond**, so positive bonds excite and negative bonds **inhibit**. Excitation
and inhibition therefore both emerge from compatibility, with no extra rule. Derive `P`, `X`
and the threshold of each material from its readings (for example the refractory length from
cohesion, and the threshold from the glow response). Matter whose threshold reads `≤ 0` fires
whenever it rests, so it is a **pacemaker**, a clock with no special case. Void never fires.
Synchronous update.

Experiments: signal speed along a wire; one-way propagation (refractoriness); a diode (asymmetric
junction); a pacemaker clock with a measured period; AND/OR/NOT built only from bonds (NOT needs
inhibition); a memory loop (a pulse circulating in a ring); a self-sustaining **spiral wave** in a
2-D sheet; and, if you can, a half adder. Be explicit about energy: nothing is conserved, so
document what the drive is and whether gameplay should make firing consume something (Prigogine).

### `toda` — the reversible wave field (Fermi–Pasta–Ulam–Tsingou, Toda, Zakharov, Chirikov, KAM)

*Escape 1 as pure mechanics.* Every matter cell is a mass on springs. It holds an integer
displacement `u` and momentum `p` (`i32`). One tick is two **integer shear maps**:
`p += F(u)` then `u += G(p)`. Each shear is exactly invertible, so the whole law is **exactly
time-reversible**, bit for bit: run forward N and backward N to reach the identical state.
`F` sums the spring forces from bonded neighbours. Stiffness comes from the bond (no bond across
void or a non-positive bond), and a **nonlinear** term (FPUT-β cubic or a Toda-like exponential
approximation in integer arithmetic) is what lets solitons and thermalisation exist. `G` scales
by the inverse mass, derived from occurrence count or cohesion. Use integer shifts for scaling
and keep everything wrapping-free (prove the bounds or saturate symmetrically, without breaking
reversibility).

Experiments: exact reversibility (digest after N forward + N backward = start); a bounded energy
drift measurement (the symplectic shadow energy); sound speed versus stiffness/mass; reflection
at an impedance mismatch; a **resonator** that rings indefinitely; a soliton crossing a long chain
without spreading (versus the linear chain, which disperses); FPUT recurrence or thermalisation;
and an honest attempt at a logic element (for example two solitons colliding, or a nonlinear
interface acting as a threshold). `poke` kicks momentum. A "rewind" gameplay verb falls out of
reversibility: show it.

### `margolus` — the reversible particle universe (Sinai billiards, Fredkin–Toffoli, Margolus)

*A different representation.* The universe is particles on the voxel lattice and nothing else
moves. Each void cell holds one bit, a particle. Matter cells are walls. One tick applies **one
bijective rule** to every 2×2×2 block of a partition, and the partition offset alternates
between even and odd ticks (the Margolus neighbourhood). The rule is the 3-D billiard-ball model.
A lone particle moves to the opposite corner of its block. Two particles at opposite corners
(head-on) are deflected to another diagonal pair by a fixed bijection that conserves momentum.
Every other pattern is unchanged, and a block that touches matter is unchanged (reflection).
The rule must be a bijection, so check that **exhaustively over all 256 block states**. The law
is then exactly reversible: apply the inverse rule with the partitions in reverse order. Use
bit-sliced or occupancy-skipping evaluation if you can, because empty blocks map to themselves.

Experiments: exhaustive bijectivity; particle-number conservation; exact reversal of a gas
expansion (the unscrambling of an egg); diffusion/effusion of a gas through a hole with a
measured rate; pressure (wall hits per tick) versus density; a mirror-routed signal; a
particle trapped in a box forever (memory/oscillator); and a billiard-ball **collision gate**
whose outputs give AND and a crossover. Discuss honestly how matter itself would arise or change.
In this law it does not, and that is the main objection. Suggest a composition with another
candidate. `poke` emits a particle.

### `zeldovich` — the synthesis: chemistry in a vibrating, conserving world

*Escape 1 for everything at once.* Combine `toda`'s reversible vibration field with `demon`'s
chemistry so that **one conserved energy** covers binding and motion. Reactions draw their
activation from the local vibrational energy at a contact, and exothermic reactions emit their
released binding as a momentum kick (sound and heat). A burning front is therefore a shock wave.
Hot regions are regions with a large incoherent vibration. A signal pulse that reaches a
metastable junction can trigger an exothermic reaction that emits a *larger* pulse. That is
**gain**, the one thing computation needs, powered by fuel. Energy accounting between the integer
vibration and integer binding must be exact. One way is to keep a small per-cell heat reservoir
that exchanges quanta with the vibration through a deterministic rule. Design it, prove the
ledger, and say where exactness had to give.

Experiments: the ledger closes over thousands of ticks; a flame front that is also a pressure
wave; an amplifier (a small pulse in, a large pulse out); a relay or threshold gate; a heat engine
or other energy-conversion device; and honest failures. This candidate is the bet that **one
quantity, energy, and one rule, deterministic exchange of conserved things across faces**, can
carry chemistry, heat, sound, signals and machines. Judge it hardest.

### `polyakov` — fields on faces and edges (Polyakov's compact lattice gauge theory, Yee)

*A different representation: the universe remembers connections, not only things.* Each cell
holds three face values `D` (flux through its −x/−y/−z faces) and three edge values `B`, plus an
integer charge `ρ` on the cell. One tick is the integer Yee scheme, written as exact shears:
`B −= curl E` with `E = (D · ε⁻¹) >> s`, then `D += curl H` with `H = (B · μ⁻¹) >> s`. Because
`div curl = 0` holds exactly in integers, **Gauss's law `div D = ρ` and `div B = 0` are preserved
exactly**, and the field part is exactly reversible. Charge moves only as a face current `J`,
which subtracts from that face's `D` and moves `J` units of `ρ` across it, so charge
conservation and Gauss's law stay exact. In matter, `J` is driven by `E` through a conductivity
derived from fit. That is the one irreversible, dissipative part, so measure it. `ε⁻¹`, `μ⁻¹` and
conductivity come from readings: choose them, justify them and keep them general.

Experiments: exact `div B = 0` and Gauss's law over thousands of ticks; wave propagation speed
with refraction at a material boundary; a capacitor (separated charge holding a field); current
in a conducting loop and induction; an LC oscillator; shielding (a closed conductor); and an
honest attempt at a switch or a logic element. Address the brief's warning head-on: is this
"electricity because reality has it", or does a gauge field earn its place as the simplest way to
remember **relations between neighbours**? `poke` deposits a pair of opposite charges or a field
kick, and must keep Gauss's law.
