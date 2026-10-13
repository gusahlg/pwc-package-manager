# Law: Polyakov (`pwc.law-polyakov`)

A candidate fundamental law for Project Watt Cubed in which **the universe remembers relations,
not only things**. Each face between two cells holds an integer electric flux `D`. Each edge shared
by four cells holds an integer magnetic flux `B`. Each cell holds an integer charge `ρ`. One tick
is Ohm's law on the conducting faces followed by the two exact integer Yee shears. The
conservation laws are identities of the cube complex (`∂∂ = 0`), so they cannot break. A library:
it registers nothing. The law is in `src/lib.rs`, the experiments in `src/tests.rs`, and the bench
in `physics-lab/laws/polyakov/examples/bench.rs`.

Every number below comes from those tests and that bench (see *Reproduce* at the end).

## 1. Inspiration

- **A. M. Polyakov, "Compact gauge fields and the infrared catastrophe" (Phys. Lett. B 59, 1975)
  and "Quark confinement and topology of gauge theories" (Nucl. Phys. B 120, 1977).** In these
  papers the gauge field is periodic, an angle rather than a number. Topological defects,
  monopoles, appear wherever a plaquette's flux winds once around the circle. From these papers
  the law takes compactness, here the `Z_65536` wrap of the machine's own `i16`, and the monopoles
  that a field overload leaves behind.
- **L. D. Landau and E. M. Lifshitz, *The Classical Theory of Fields* (Course vol. 2, 1941).**
  Here the field is a carrier of interaction with its own energy and momentum, and charge
  conservation is the integrability condition of Maxwell's equations. The law takes conservation
  as a consequence of structure, never as a rule that has been added.
- **V. I. Arnold, *Mathematical Methods of Classical Mechanics* (1974).** Arnold treats symplectic
  maps and the leapfrog as a composition of shears. The law takes the shear form `B −= C·E(D)`,
  `D += Cᵀ·H(B)`: each step is invertible whatever the rounding inside `E` and `H`, so the field
  part is a bijection.
- **Science fiction: Arkady and Boris Strugatsky, *Roadside Picnic* (1972).** In the Zone,
  invisible fields fill the empty space between things, and the stalker finds them by throwing
  bolts. The poke is that bolt: a charge pair thrown into the world, joined by its flux string.
  The heat map is the stalker's eye. **Stanisław Lem, *Solaris* (1961)** contributes the ocean
  that remembers what touched it. Here the space between blocks remembers what passed through it.

From outside that tradition: K. S. Yee (1966) for the staggered grid. L. Kronecker (1857) for the
theorem that forces rounding (section 5). K. Wilson (1974) for loops of link variables as the
observables of a gauge theory.

## 2. What the universe remembers

| where | what | type | bytes per cell |
|---|---|---|---|
| each cell's −x, −y, −z face | electric flux `D` through it (positive along +axis) | `i16`, wrapping | 6 |
| each cell's x, y, z edge through its (−,−) corner | magnetic flux `B` | `i16`, wrapping | 6 |
| each cell | charge `ρ` | `i16`, wrapping | 2 |

That is **14 bytes per cell** of state. `state_bytes_per_cell` says 14. The law also keeps a cache
of the matter: `ε⁻¹` per face (3 bytes per cell) and the list of conducting faces (12 bytes per
conducting face). It keeps 6 bytes per cell of scratch (`E`, then `H`, within a tick) and 1
liveness byte per row of cells.

`ρ` is redundant. Gauss's law makes it equal to `div D` at all times, and nothing in the dynamics
reads it. It is kept because the brief asks for it, and because keeping it lets the experiments
check that the update code keeps the constraint. A host could drop it and save 2 bytes.

**The box is the whole universe.** A face with a cell on only one side relates nothing, so it
holds nothing. The same goes for an edge missing one of its four cells. Physically this is a
perfect magnetic wall (normal `D` and tangential `H` vanish on the box surface), but it costs no
special case: the out-of-box edges are zeros, and the curl of zeros is zero.

## 3. The law

One tick, in a fixed order (every pass works in place and reads only what the previous pass
finished):

1. **Ohm**, on each conducting face (cell `j` on its − side, `i` on its + side), with its own flux
   `D`: `J = sgn(D)·round((|D| − T)⁺ · g / 256)`. Then `D −= J`, `ρ_j −= J`, `ρ_i += J`. Since
   `g ≤ 256`, `|J| ≤ |D|`: a face can discharge fully, never past zero. Joule loss `J·E` is
   counted in the ledger. This is the only lossy, irreversible operator.
2. **E**: `E = round(D · ε⁻¹ / 256)` on every face.
3. **Faraday**: `B −= curl E` on every edge whose four cells exist.
4. **H**: `H = round(B · μ⁻¹ / 256)` on every edge.
5. **Ampère**: `D += curlᵀ H` on every face. Faces on the box surface always receive zero.

Every `round` is to nearest with ties toward zero, so it is never larger in magnitude than the
exact value. Arithmetic on `D`, `B` and `ρ` wraps mod 2¹⁶. `curl` takes a face's `E` to the
circulation around each edge, and `curlᵀ` is its exact transpose; test
`ampere_is_exactly_the_transpose_of_faraday_and_the_box_surface_holds_nothing` compares the two
matrices entry by entry. Two unconditional identities follow:

- `div curlᵀ = 0` on every cell, so **Gauss's law `div D = ρ` holds in every cell, every tick.**
  Ohm keeps it face by face, and so does the poke.
- `div curl = 0` at every vertex surrounded by eight cells, so **`div B = 0` holds at every such
  vertex.**

Both are exact in `Z_65536`. As plain integers they are exact while no field has wrapped.

**Constants.** No material is named. All values are in 1/256.

| constant | where | formula | why |
|---|---|---|---|
| `ε⁻¹` of a cell | cell | `128·256 / (256 + max(0, light))` | The light probe is what the game already reads as transparency. Attraction to it is polarisability: clear matter slows the field. Void reads light 0, so it is the vacuum. |
| `ε⁻¹` of a face | face | mean of its two cells' `ε⁻¹` | Two half-cells in series. |
| `μ⁻¹` | edge | 128 everywhere | An edge belongs to four cells and to no pair, so no reading belongs to it. One constant fewer. |
| `g` (share of a face's flux discharged per tick) | face | `min(clamp(glow_a, 0, 256), clamp(glow_b, 0, 256))` | Carriers must exist on both sides. Matter that glows in the game carries charge here, so a lamp filament is a conductor. 256 is the perfect-conductor limit. Void has no carriers. |
| `T` (breakdown flux) | face | `max(0, −bond(a, b))` | Incompatible neighbours resist the passage of charge in proportion to their incompatibility. A compatible face is Ohmic. |

Field units are the Q8 fit units by fiat: a barrier of 430 means a flux of 430. That is the one
scale choice in the law.

**Stability by construction.** In exact arithmetic the leapfrog is stable when
`ε⁻¹·μ⁻¹·λ_max(curlᵀcurl) ≤ 4`, and `λ_max = 12` on the cubic lattice. So it needs
`ε⁻¹·μ⁻¹ ≤ 1/3`. The vacuum has the largest `ε⁻¹` any face can get (matter only lowers it), and
`μ⁻¹` is universal, so `ε⁻¹·μ⁻¹ ≤ (128/256)² = 1/4` holds for every face that any fit can
produce. Ties-toward-zero rounding never raises a coupling above its coefficient. Ohm needs
`g ≤ 256`, which the clamp guarantees. So stability is guaranteed by integer construction for all
fit-derived constants. Test `coefficients_near_one_break_the_cfl_bound_and_blow_up` shows what
happens without that construction.

**Compact, not non-compact: why `Z_65536`.** A non-compact field needs either unbounded integers
or saturation. Saturation breaks `div D = ρ` the moment one face clips. Wrapping keeps every
identity exact, but mod `N`. The state space becomes finite and the field step is a permutation
of it: nothing can overflow, there is nothing to prove about bounds, and every orbit of the
lossless part is exactly periodic. The price is that the law changes character beyond ±32768. A
plaquette that wraps is a monopole–antimonopole pair whose Dirac string is the wrap itself,
invisible to the law. That is Polyakov's compact `U(1)`, discretised as `Z_N`. `i16` also halves
memory compared with `i32`, and it puts the monopoles within reach of play (section 4) instead of
at 2³¹. The poke clamps its strength to ±32767.

**The poke** creates `+s` at the touched cell and `−s` at its +x neighbour, with `s` units of flux
on their shared face. That is the smallest Wilson line, and Gauss's law holds before and after.
Chained pokes extend the string, which is how the experiments charge plates and loops. `kick`
(a field kick) changes the four edges around a face as if that face had held a field for one tick,
which keeps `div B = 0`. `step_back` runs the field part backwards. `unstep` does the same and
returns `true` only when no face conducts.

## 4. What emerges

Each item gives the test name, then its measurement.

- **Exact conservation** (`gauss_law_and_div_b_hold_exactly_every_tick`). A 16×14×12 box holds a
  conductor slab, a dielectric block, an incompatible conductor junction and void. 3000 ticks with
  13 pokes and kicks give 0 Gauss defects and 0 `div B` defects in every cell and vertex, every
  tick. Total charge stays 0. Two runs give the same digest (`a2cdb727cd4642d9`). Joule loss over
  the run is 2.4·10⁹.
- **Exact reversal of the field** (`the_field_runs_backwards_exactly_without_currents`). In a
  20×18×16 box with a dielectric block, two pokes and a kick, 2000 ticks forward and then 2000
  `unstep`s return the starting digest (`68513c302b1f52c8`) bit for bit.
- **Waves at a speed set by the constants**
  (`waves_travel_at_half_a_cell_per_tick_and_slow_by_root_epsilon`), measured on a 2-D sheet
  400×480. A pulse in vacuum travels **0.4985 cells/tick**; the continuum value is
  `√(½·½) = 0.5`.
- **Refraction**, same test. A dielectric found by search reads light 812, so `ε⁻¹ = 30/256` and
  `n = 2.066`. The measured speed inside is **0.2429** against a predicted 0.2421, and the
  measured index is 2.052. At normal incidence, **energy reflected is 0.122 (Fresnel 0.121) and
  energy transmitted is 0.878 (Fresnel 0.879)**. The reflected B amplitude is 0.339 (Fresnel
  0.348). Nothing in the law mentions reflection.
- **Capacitor** (`a_capacitor_holds_its_charge_and_a_dielectric_raises_its_capacitance`). Two
  16×16 resistive plates with a 4-cell gap are charged by 64 Wilson lines. Plate charge is
  **25600, exactly, every tick for 3000 ticks**: void never conducts. The field inside a plate
  reads 3, against about 33 per face in the gap. Measured `C = Q/V = 155.0`, which is the ideal
  `A/Σε⁻¹ = 102.4` plus 52.6 of stray capacitance. Filling the gap with the dielectric adds
  **159.9**; the series-face prediction is 161.9.
- **LC oscillator** (`an_lc_loop_rings_and_a_dielectric_slows_it`). A square conductor loop (2×1
  wire, 16×16) has a 2-cell gap, and a flux string across the gap charges it. The charge sloshes
  between the halves of the loop: **24 swings in 2400 ticks, period 200.9 ticks**, and the late
  peak `|Q|` is 9015 against 6000 at the start. With dielectric in the gap the period is 208.8
  ticks. With the whole loop embedded in dielectric it is **344.0 ticks**, ×1.71. The wire's own
  faces keep the vacuum's `ε`, so the ratio is below `n = 2.07`.
- **Induction and Lenz's law** (`a_changing_current_induces_an_opposite_one_in_a_closed_ring`). A
  closed ring two cells above the ringing loop carries a current anti-correlated with the
  primary's: **correlation −0.393**, rms ratio 1.06. The ring's net charge stays exactly 0 every
  tick, because it is an island.
- **Shielding** (`a_closed_conductor_shields_its_inside`). A hollow 10³ shell sits in a 28³ box,
  with a ±3000 pair outside it whose string is routed around it. Mean cavity field energy over
  ticks 3000–4000: no wall 7,859,037; dielectric wall 5,494,916; resistive wall 412,005 (19× less);
  **conductor wall 0** (under one energy unit on every tick). The static centre flux is 6.39,
  5.31, 0.03 and **0.00**.
- **Threshold gate: a switch made only of bonds**
  (`an_incompatible_junction_is_a_threshold_gate`). Two conductors whose bond is −430 touch across
  9 faces, so `T = 430`. Inputs are Wilson lines of `q` from a ground plate onto conductor A.
  **AND at q = 2800: 00→0, 10→0, 01→0, 11→972.** **OR at q = 4000: 0, 502, 362, 2184.** A sweep
  over q = 1000…12000 never delivers more charge than came in.
- **Permanent magnets from compactness**
  (`a_field_driven_past_the_wrap_tears_into_a_monopole_pair_that_stays_a_magnet`). Two opposite
  kicks of 20000 drive one edge to −40000, which wraps to +25536. The result is **exactly one
  monopole pair (`div B = ±65536`, sum 0)**, still present after 2000 ticks. The edge's late
  mean B is **21833**: a static magnetic field held by topology. The same experiment at 10000
  makes no monopoles, and the late mean B is −0.8, because the kick rings away.
- **Stability is the construction, not luck**
  (`coefficients_near_one_break_the_cfl_bound_and_blow_up`). With `ε⁻¹·μ⁻¹ = 1/4` the plain energy
  stays within 2.8× of its start over 300 ticks; that is D↔B exchange, also present in exact
  arithmetic. With `(255/256)²` it grows **20,306×** and leaves 1618 monopoles of wrapped noise.

## 5. What it cannot do, or does badly

- **Rounding heats the vacuum, and nothing can stop it in a closed box**
  (`rounding_heats_the_vacuum_slowly_whatever_the_amplitude`). In a 16³ vacuum after one kick, the
  shadow energy (the quantity exact arithmetic conserves) drifts by **3289 to 3652 per tick
  whatever the kick: 100, 1000 or 10000**. That is 0.89 energy units per cell per tick, linear in
  time: a random walk of rounding errors. A kick of 10000 drifts by 14.6 % in 8000 ticks, and a
  kick of 100 is buried in noise. The rounding rule does not matter: truncation and half-away
  give the same numbers, because every vacuum product is a multiple of 128. The cause is
  structural, not a bug:
  - exact integer coefficients violate the stability bound (shown above);
  - a stable, exactly linear integer map would be in GL(n, ℤ) with every eigenvalue on the unit
    circle. By Kronecker's theorem (1857) those eigenvalues are roots of unity, so the map would
    be of finite order, a lattice permutation rather than a wave equation with a continuous
    dispersion relation. This is an argument, not a proof for every scheme.

  So any integer Yee-like law must round, and rounding inside a bijection diffuses energy
  upward. In the bench's terrain the lossy soil carries the heat away and the noise plateaus
  (section 6).
- **The vacuum is never quiet once disturbed.** Waves in void never decay, and the noise above
  keeps every touched row nonzero. 200 ticks after 64 pokes, 87 % of the bench box is live, then
  100 % for good.
- **No gain.** Fields superpose linearly. The only nonlinearities are the junction barrier,
  rounding and the wrap. A gate passes 0.17 (AND) or 0.27 (OR) of its input charge, so gates
  cannot drive gates and no computer can be built from this law alone.
- **No diode, by construction.** The barrier acts on `|D|`, so every junction is symmetric.
- **Matter never changes.** No force acts on blocks and no current moves an element. As a
  standalone law it is a second universe living in the gaps of the first.
- **Light is slow.** At 0.5 cells/tick and 20 Hz, a signal crosses a 64-block room in 6.4 s. That
  suits "signals" and "power" in gameplay, not light.
- **Charge in air is permanent.** Void has no carriers, so a poke in air leaves a static dipole
  forever. A natural fix, breakdown of air under a strong field, would need a void reading the
  law does not have.
- **Ohm is local.** Charge crosses at most one face per tick, so a conductor relaxes at the speed
  of the field and no faster.
- **Plane waves are not modes of the box.** The magnetic wall bends any uniform wave near the
  walls. The wave test therefore measures only the middle 80 rows, before the wall disturbance
  arrives.

## 6. Cost

Bench: a 64³ box (262,144 cells), 50 % matter. Insulating rock holds a conductor vein and
dielectric crystals, six layers of lossy soil lie on top, and air fills the rest. The bench
reports 71,534 conducting faces (3.3 bytes per cell of cache).

| condition | ns per cell-tick | cells per second | active |
|---|---|---|---|
| at rest | 0.14 | 7.0·10⁹ | 0 % |
| busy, first 200 ticks after 64 pokes | 5.55 | 1.8·10⁸ | 86.6 % |
| busy, ticks 200–1200 | 6.12 | 1.63·10⁸ | 100 % |
| every row forced live | 6.10 | 1.64·10⁸ | 100 % |

Memory is 14 bytes per cell of state, 3 per cell of `ε⁻¹` cache, the conducting-face list, 6 per
cell of scratch and 1 per row of cells. **It is a field law, and fields are dense.** Every live cell
costs the same whatever happens there. Rows of all-zero fields are skipped, which is where the
0.14 ns at rest comes from, but the vacuum never returns to zero (section 5).

Long run on the same box (energy, Joule so far, rms field per component):

| tick | field energy | Joule | rms field |
|---|---|---|---|
| 2,000 | 5.17·10⁸ | 7.1·10⁸ | 26 |
| 21,000 | 6.98·10⁸ | 6.2·10⁹ | 30 |
| 41,000 | 7.84·10⁸ | 1.22·10¹⁰ | 32 |
| 81,000 | 1.01·10⁹ | 2.43·10¹⁰ | 36 |
| 121,000 | 7.83·10⁸ | 3.68·10¹⁰ | 32 |

The noise plateaus: from about 40,000 ticks on, the soil's Joule loss (about 3.1·10⁵ per tick)
balances the rounding heat, at a floor of 32–36 flux units rms against pokes of 4000. Gauss and `div B` defects are 0 at the end.

What a world-scale host would need:
- **Chunking**: per-chunk arrays plus a one-layer halo of `E` (before Faraday) and `H` (before
  Ampère) exchanged across chunk faces every tick. Fields cross chunks freely, so a chunk border
  must not be a wall.
- **An absorbing sponge** where the simulated region ends. Without one, radiation and rounding
  heat accumulate (section 5). A sponge breaks exact reversibility only at the edge.
- **Activity**: the row skip is free and correct, but rows only go quiet in lossy matter. Most
  worlds would be busy wherever anything ever happened.
- **Server authority and determinism**: integer, fixed order, no hashing, so lockstep is
  possible.
- **Network**: once excited, every field changes every tick, so deltas are useless. Clients must
  run the same law in lockstep from the same pokes, or receive only a coarse visual (the `show`
  heat map).
- **Saves**: 14 bytes per cell for every chunk with fields. Most of a quiet world is zero and
  compresses well; an excited one does not.
- **Matter edits**: the `ε⁻¹` cache and conducting-face list of an edited cell must be rebuilt.

## 7. Gameplay

A player would discover that:
- glowing matter conducts and dull matter insulates;
- clear crystals slow and bend the invisible field (refraction, partial mirrors);
- two conductors that "dislike" each other form a spark gap, which conducts only when pushed hard
  enough.

From those they can build:
- capacitors that hold charge forever on an insulated island;
- LC loops that ring with a period of about 200 ticks (10 s) and slow down inside crystal;
- wireless coupling between stacked loops;
- Faraday cages, which shield perfectly;
- threshold AND and OR gates;
- permanent magnets, made by overloading a field until it tears.

**The poke** throws a charge pair joined by a flux string. Repeated pokes extend the string into a
line of force a player can lay like a wire.

What would confuse or frustrate:
- everything is invisible without the heat map;
- the field is slow;
- the noise floor makes everything faintly alive;
- charge thrown into air stays forever;
- **gates do not cascade**, so there is no "redstone computer" to build;
- nothing the field does ever changes a block.

## 8. Verdict

| criterion | score | why |
|---|---|---|
| emergence | 3 | Refraction with Fresnel's coefficients, capacitance, LC ringing, Lenz, perfect Faraday cages and topological magnets all come from one rule with no special case. All of it is linear Maxwell, which was known in advance. |
| generality | 4 | Every configuration gets `ε`, carriers and a barrier from its readings. Void is not special-cased. `μ` is universal. |
| expressive power | 2 | No gain (gates pass 17–27 % of their input). Matter is never changed. It cannot compute alone. |
| elegance | 4 | Conservation is topology (`∂∂ = 0`), the step is two shears, and the machine's wrap is the gauge group, making it a finite reversible automaton. Ohm is a bolt-on, three probe mappings are choices, and rounding heats. |
| efficiency | 3 | 6.1 ns/cell-tick dense, 0.14 at rest, 14 B/cell. Dense once excited, and never quiet again without lossy ground. |
| enjoyment | 2 | An electrical-engineering sandbox (cages, magnets, resonators) with invisible, slow fields and no cascading logic. |
| extensibility | 4 | It is the natural transport layer for another law. Joule heat can feed `demon`'s ledger, current can drive transfers (electrolysis) and `J×B` can kick `toda`'s momentum. Charge is `div D`, so any coupling that moves charge keeps Gauss's law for free. |

**On the brief's warning: is this electricity because reality has it?** For the record, the case
that it is more than that is real. The fit law already lives on faces: `bond(a, b)` is a static
relation between two cells. `D` is the same face remembering what has passed across it, and
Gauss's law makes a cell's charge nothing but the balance of its relations. `B` on an edge is the
memory of the loop of four relations around it, a Wilson loop, and induction is that loop
remembering how the relations around it changed. No conservation was added. All of it is
`∂∂ = 0` on the cube complex the voxel world already is, and compactness makes it a finite
reversible machine. As a *representation* (state on faces and edges with conservation by
topology), it is the most elegant thing in this lab.

As a *fundamental law* for this game, it does not earn its place on its own. It is linear, so
nothing amplifies. It is blind to the elements, so it never changes matter. It needs three
probe-to-constant mappings that are choices, not consequences. Its integer form cannot be both
exact and quiet (Kronecker), so it slowly boils unless matter drinks the heat.

**Strongest objection: it is a second universe running in the gaps of the first.** Without a
coupling that lets currents move elements, or a source of gain, it is electricity because reality
has it. It should survive only as the field half of a synthesis, for example currents that drive
selective transfer (electrolysis) with Joule heat paid into a conserved ledger. It should not be
the law of the world by itself.

## Reproduce

From `physics-lab/`, with any `CARGO_TARGET_DIR`:

```
cargo test --release -p pwc_law_polyakov -- --nocapture
cargo run --release -p pwc_law_polyakov --example bench   # about 3 minutes (121k-tick long run)
cargo clippy -p pwc_law_polyakov --all-targets
```

## Licence

`Apache-2.0 OR MIT`, at your option.
