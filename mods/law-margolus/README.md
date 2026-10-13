# Law: Margolus Billiards (`margolus`)

A candidate fundamental law for Project Watt Cubed: **a reversible particle universe**. Matter is
walls. The only thing that moves is particles, one bit per open cell. One tick applies one block
rule to every 2×2×2 block of a partition whose offset alternates between ticks. The rule is not
chosen but **derived** from the symmetry of the cube, and every block map is its own inverse, so the
whole history can be run backward bit for bit.

Every number below comes from a test in `src/tests.rs` (run with `--nocapture` to see it) or from
the benchmark `physics-lab/laws/margolus/examples/bench.rs`. Test names are given in brackets.

## 1. Inspiration

- **Nikolai Krylov**, *Works on the Foundations of Statistical Physics* (1950, posthumous): the
  relaxation of a gas comes from the instability of colliding hard spheres, not from added
  randomness. Taken: no dice anywhere; statistics must come out of deterministic collisions.
- **Yakov Sinai**, the ergodicity of hard-sphere gases (announced 1963) and *Dynamical systems with
  elastic reflections* (1970); **Kolmogorov** (1958) and **Sinai** (1959) entropy. Taken: billiards
  with walls as the whole of mechanics, and damage spreading as the measure of chaos
  [`one_extra_particle_spreads_through_the_gas`].
- **Fredkin and Toffoli**, *Conservative Logic* (1982), and **Margolus**, *Physics-like models of
  computation* (1984): the billiard-ball computer and the partitioned automaton, in which
  reversibility is free because each block map is a bijection. Taken: the neighbourhood, the
  collision gate and the exhaustive bijectivity check.
- **Hardy, Pomeau and de Pazzis** (1973, the HPP lattice gas) as a warning: a square lattice gas
  with too few collisions conserves momentum along every line. Section 5 proves the 3-D analogue
  here.
- Science fiction: the **Strugatsky brothers**, *Monday Begins on Saturday* (1965), whose Janus
  Poluektovich lives in *countermotion*, from the future into the past. Here that is one call,
  `reverse()`, after which the world runs its own history backward. **Stanisław Lem**, *Summa
  Technologiae* (1964), on building worlds from rules rather than from props: the law has one
  derived table and one threshold, and no material has a name.

## 2. What the universe remembers

- **Per cell: one bit**, a particle or none (open cells only). A particle's velocity is not stored.
  It is the parity of the cell against the partition offset: along each axis, +1 when the
  coordinate has the offset's parity and −1 otherwise. So eight velocities `(±1, ±1, ±1)` exist,
  each living on its own sublattice. `state_bytes_per_cell` reports 1 (the interface's whole-byte
  ceiling of one bit).
- **Global: one bit**, the partition phase. `reverse()` flips it.
- **Per face: nothing.** Matter is read once at construction into two derived bitmaps (walls and
  clear cells), aligned per phase. They are a cache of the matter, not law state.

## 3. The law

A block's eight corners are `c = dx | dy<<1 | dz<<2`. A particle at corner `c` has velocity +1 on
every axis whose bit is 0. A block state is the set of occupied corners (one byte). The rule
(`src/rule.rs`) is one 65 536-entry table indexed by `(wall corners, particles)`:

1. **Empty space: collide, then fly.** Free flight `F` sends every particle to the opposite corner
   `c ^ 7`. Before it, the collision `G` replaces the block's set of velocities by another set with
   the same particle number and momentum. `G` is defined as **the unique bijection that conserves
   number and momentum, commutes with all 48 symmetries of the cube, and fixes no state that such a
   bijection could move**. The code finds it by searching each momentum class for derangements
   that commute with the symmetries fixing that momentum. Each class has exactly one or none
   [`the_collision_is_forced_by_the_symmetry_of_the_cube`]:
   - two particles whose velocities agree on one axis and are opposite on the other two (a
     *glancing* pair, P = ±2·e): 6 classes. The pair leaves on the other two velocities that keep
     the shared axis. This is the 2-D billiard-ball collision carried along the shared axis;
   - four particles: all 19 classes with more than one state (including the 8-state P = 0 class,
     where two tetrahedra swap and two head-on pairs leave on the other two diagonals);
   - six particles: the 6 glancing classes of holes (the rule is particle–hole symmetric);
   - **head-on pairs (P = 0) and all odd numbers: no isotropic collision exists**, so they fly on.

   80 of the 256 states collide [`every_block_map_is_a_bijection_and_its_own_inverse`]. `G` is an
   involution and commutes with `F`, so the empty-space map `F∘G` is an involution too.
2. **Walls: mirror.** In a block that holds wall corners `W`, nothing collides. Each particle moves
   to `c ^ m(W)`, where `m(W)` holds every axis under whose reflection `W` is symmetric. A particle
   keeps going along those axes and turns back along the others. A flat wall is a specular mirror,
   an inside edge reflects two axes, and a lone wall corner or a diagonal pair of wall corners
   reflects all three. Diagonal gaps therefore never leak. The map is an involution.
3. **Clear matter: flight only.** In a block with clear matter and no wall, `F` alone applies.
4. **The box is closed.** Outside the box is wall.

**Constants and their derivation.** There is one: a configuration is **open** (particles enter it)
when its light-probe response exceeds `CLEAR = 2 · 256` (Q8 of `QUANTUM`). That is exactly where
`material::observe` begins to read a configuration as transparent, so what light sees through,
particles fly through. Every other configuration is a wall. The collision has no constant at all.

**Why matter can do so little more.** A cell's readings are scalars, and scalars carry no
direction, so a rule they select must commute with the cube's symmetries. The only isotropic maps
of a lone particle are *keep flying* and *reverse* [`the_collision_is_forced_by_the_symmetry_of_the_cube`,
"isotropic maps of a lone particle"]. Per class, the only isotropic collision choices are *the
derived one* or *none*. A scalar reading can therefore choose only **mirror, collide or pass**,
plus geometry (the arrangement of walls), which is what makes mirrors directional. A *handed*
reading (a pseudoscalar) would not help: it admits 3 different collisions in each 4-state class of
four particles, so it does not select one. This law uses all three options: opaque matter is a
mirror, void collides, and clear matter passes.

**Order and determinism.** Blocks of one partition are disjoint, so the update is synchronous and
in place. The evaluation order cannot matter. Arithmetic is integer and bitwise only.

**Reversal.** `unstep()` applies the same partition again (every block map is an involution) and
is the exact inverse of `step()`. `reverse()` makes the next tick repeat the last partition, which
reverses every velocity, so stepping forward replays the history backward.

**Deviations from the brief, and why.**
- *"Two particles at opposite corners are deflected by a fixed bijection."* There are 9 such
  bijections. Each is one rotation of the pair: three are 180° turns about a coordinate axis
  (achiral, commuting with 16 of 48 symmetries) and six are 90° turns (chiral, commuting with 8)
  [`the_collision_is_forced_by_the_symmetry_of_the_cube`]. None is isotropic, so every choice
  makes one axis special and two thirds also pick a hand. The derived rule leaves head-on pairs
  alone and deflects glancing pairs, which *is* the 2-D billiard-ball collision in every plane at
  once.
- *"Every other pattern is unchanged, and a block that touches matter is unchanged."* "Unchanged"
  means a particle stays put, so its velocity reverses (bounce-back). That breaks momentum
  conservation, cannot route a signal around a corner
  [`a_mirror_routes_a_signal_and_bounce_walls_only_return_it`], and **freezes every two-cell slab**,
  because every second partition touches the floor or ceiling: a particle never gets more than 1
  cell from its source, against 13 with mirrors [`bounce_walls_freeze_a_thin_slab_and_mirrors_do_not`].
  The literal rule is kept as `Rule::LITERAL` (head-on pairs turn 180° about z, the achiral choice,
  so it stays an involution), and section 5 measures what it buys.

## 4. What emerges

| Phenomenon | Measurement | Test |
|---|---|---|
| Exact bijection | all 3⁸ = 6561 (wall, particle) block states of each of the 4 rule variants map one-to-one, and applying a map twice is the identity | `every_block_map_is_a_bijection_and_its_own_inverse` |
| Implementation = definition | word-parallel engine equals a cell-by-cell evaluation on 7 box shapes (1³ … 130×3×5, random walls and clear matter), 4 rules, 40 ticks | `the_word_engine_matches_the_cell_definition` |
| Conservation | 1686 particles: momentum exact over 14 ticks of flight with 652 collisions; number constant over 3000 ticks and 318 654 wall hits; per tick, the momentum change ≤ 2 × wall hits | `particles_are_conserved_and_momentum_changes_only_at_walls` |
| The egg unscrambles | 6471 particles expand through a 6×6 hole; coarse entropy (8³ cells) rises 3.464 → 3.746 → 3.926 → 4.028 → 4.086 nats over 600 ticks; 600 × `unstep` restores every bit (equal digest), and `reverse` + 600 forward ticks does too | `a_gas_expansion_unscrambles_exactly` |
| Effusion | ρ = 0.301, hole of 16 cells: early flux 0.650/tick against the free-molecular ρA/2 = 2.411, so the transmission is κ = 0.270 (the hole's edge blocks reflect). The imbalance decays exponentially with τ = 2537 ticks against V/(κA) = 3205 (−21%), and is 210 of 4166 after 8000 ticks | `effusion_through_a_hole_has_a_measured_rate` |
| Ideal gas law | wall hits per tick per face cell p = ρ/2 at all 8 densities from 0.022 to 0.599: p/ρ = 0.5000, spread 0.000%. This is exact, which is the kinetic-theory value for unit speed (half the particles at a wall move into it) | `pressure_is_proportional_to_density` |
| Memory and oscillator | one particle in an open 3×5×7 box returns with period exactly lcm(2·3, 2·5, 2·7) = 210 from 3 different starts, and again after 100 more periods. An empty box stays empty: one stored bit | `a_particle_in_a_box_returns_forever` |
| Mirror-routed signal | a particle reaches a room hidden behind a screen via a mirror plane at tick 27 (13 ticks to the mirror, then y climbs to the ceiling and descends: 26 moves and 1 turning tick). Under bounce-back it never arrives in 2000 ticks | `a_mirror_routes_a_signal_and_bounce_walls_only_return_it` |
| Collision gate (genuine 3-D, no confinement) | inputs A (+,+,+) and B (−,−,+) give outputs A·¬B, ¬A·B, A·B, A·B: truth table 00→0000, 10→1000, 01→0100, 11→0011 | `the_collision_gate_computes_and` |
| Crossover | (+,+,+) with (+,−,+) and (+,+,+) with (−,−,−) share a block and both fly on, with 0 collisions | `crossing_paths_pass_through_each_other` |
| Matter switches logic | the same gate with its collision site inside clear matter passes both inputs straight through (outputs 1100): clear matter is a crossover medium, and particles are seen inside it | `clear_matter_turns_the_gate_into_a_crossover` |
| Chaos (Krylov, Sinai) | one extra particle in a 30% gas: cells that differ 5, 9, 23, 149, 1383, 11 821, then 13 669 … 13 755 at ticks 1 … 256. This is light-cone growth (×8–9 per doubling), not exponential, and it saturates at the uncorrelated 2ρ(1−ρ)V ≈ 13 763. 256 unsteps bring it back to the single particle | `one_extra_particle_spreads_through_the_gas` |
| Player action | `poke` emits 27 particles at radius 1, puts none into walls, emits only into open cells, and is counted in the `emitted` ledger; matter is unchanged after 500 ticks | `poke_emits_particles_and_the_ledger_counts_them` |

From the gate's truth table: with A held at 1 (a constant stream), the A·¬B output is NOT B, and
the two A·B outputs are two copies of B (fan-out). With the trivial crossover, this is
Fredkin–Toffoli conservative logic, which is universal given constant streams and sinks.

## 5. What it cannot do (or does badly)

- **The derived gas never diffuses in projection.** This is a theorem, and it is tested. If
  momentum and number are conserved in every block, each block sends exactly as many particles to
  each x half as free flight would. The x-profile of the gas therefore evolves exactly as if
  nothing ever collided, and the same holds for y and z, with flat walls included. In 200 ticks
  with 35 325 collisions, all three 1-D density profiles equal those of a collisionless twin at
  every tick, and the spread grows at 1.000000 × the collisionless rate at densities 0.05, 0.30
  and 0.70 [`collisions_are_invisible_in_every_projection`]. The exact ideal gas law is the same
  fact seen from a wall. Collisions only reshuffle which y and z go with which x: the gas has
  pressure but no viscosity or diffusion in the hydrodynamic sense. This is the 3-D analogue of
  HPP's spurious invariants, and it holds for **any** momentum-conserving rule with one speed on
  this lattice. Escapes would be rest particles, a second speed, or breaking momentum.
- **Breaking momentum buys diffusion and costs symmetry.** Under the literal rule, a dense cloud's
  spread grows at 0.874/0.825/0.915 (x/y/z) of the collisionless rate at density 0.05, 0.24 at
  0.30 and 0.018 at 0.70. Momentum is no longer conserved
  [`the_literal_rule_diffuses_but_breaks_momentum`]. No consistent anisotropy was measured (z grew
  fastest at 0.05 and slowest at 0.30), so no claim is made. Its pressure (with mirror walls) is
  p/ρ = 0.503, 0.499, 0.502 at ρ = 0.05, 0.30, 0.60.
- **Matter never arises or changes.** The law reads matter once and never writes it (asserted
  after 500 ticks in `poke_emits_particles_and_the_ledger_counts_them`). Particles cannot stick,
  because a reversible law has no attractors: a trap that keeps particles forever could, run
  backward, never have filled. Binding needs something to carry the released energy away, which
  is chemistry this law does not have. **This is the main objection.**
- **Generality is thin.** Of all a configuration's readings, the law uses one bit (open or not),
  plus "clear" for open matter. Two rocks behave identically. By the symmetry argument in section 3,
  scalar readings *cannot* do much more inside a block rule.
- **No gain.** Particle number is conserved, so a signal is never amplified. Fan-out and NOT cost
  a supply stream, and garbage outputs must be routed away. A computer needs a power source the
  law does not provide.
- **Fragility and timing.** A gate works only if its inputs meet in the same block on the same
  tick, and one stray particle decorrelates 86% of a 32³ gas within 32 ticks (chaos test).
  Circuits must be shielded and clocked to the tick.
- **Effusion is only κ = 0.27 efficient** for a 4×4 hole in a 1-cell wall, because the hole's
  edge blocks reflect. The exponential fit was 21% faster than the effusion estimate made from
  the first 40 ticks. Both are measured, and neither is tuned.

## 6. Cost

Benchmark: 64³ box, 51.5% matter (rock and soil under a clear crust, gentle hills), release build.

| Scene | Particles | ns / cell-tick | Cells / s | µs / tick | `unstep` ns / cell-tick | Active |
|---|---|---|---|---|---|---|
| At rest | 0 | 0.022 | 46 G | 5.7 | 0.022 | 0% |
| Busy (8 pokes of radius 3) | 2 695 | 0.110 | 9.1 G | 28.9 | 0.109 | 7.6% |
| Dense gas (30% of cells) | 40 695 | 0.38–0.39 | 2.6 G | 99–102 | 0.39 | 51.4% |
| Every open cell full | 135 274 | 0.599 | 1.7 G | 157 | 0.602 | 57.5% |

Building the universe, including deriving the rule table from the symmetry search, takes 1.1 ms.

- **How it is fast:** bits along x in `u64` words. Each word covers 32 blocks, and a word with no
  particle is skipped. Blocks with one particle (or any odd number, under the derived rule) and no
  wall move with four word-wide shifts and swaps. Only blocks with an even number ≥ 2 of particles,
  or with walls, are looked up in the table, one byte at a time. `unstep` costs the same as `step`.
- **Memory:** 1 bit per cell of state. The derived wall/clear caches are 4 bitmaps × (row + 1
  word), which is 8 bits per cell at nx = 64 (4 bits with a tighter alignment).
- **World scale:** a 64³ chunk is one word per row, so a chunk is 32 KiB of state. Blocks straddle
  chunk borders every second tick, so a host needs a 1-cell halo from each neighbour. An empty
  chunk costs nothing if the host tracks per-chunk particle counts. Saves are the particle bits
  plus the phase bit (sparse and compressible). Busy regions change every bit every tick, so
  network deltas are useless there. The law is deterministic and cheap, so peers should simulate
  in lockstep from the same pokes, and the server should send pokes rather than state. Exact
  reversibility gives a server **exact rewind** for lag compensation and replays.

## 7. Gameplay

- **The verb:** `poke` emits a burst into every open cell within radius `strength − 1` (a negative
  strength vacuums). A burst flies apart in all eight diagonal directions.
- **What players would discover:** gas fills rooms and leaks through holes at a measurable rate.
  Opaque blocks are mirrors and clear blocks are windows. A sealed room keeps a particle bouncing
  forever with a period set by its size, which makes a clock. Two beams that meet glancingly
  deflect, while beams that cross head-on or at right angles ignore each other.
- **What players would build:** billiard-ball logic (AND, NOT and fan-out from supply streams),
  delay lines and clocks from boxes, and crossovers made of clear matter. With mirror walls a
  particle keeps moving even in a two-cell-thin slab (it reached 13 cells, against 1 under
  bounce-back), so circuits can be layered.
- **The joy:** **rewind**. `unstep` and `reverse` are exact, so time travel is a real verb. A
  player can scramble a room and unscramble it, and a puzzle can ask for the poke that produces
  some state.
- **Frustrations:** a particle's direction is set by the cell it is born in, not chosen. Timing
  must be exact to the tick. One stray particle wrecks a circuit. Nothing in the world reacts to
  particles: they do not heat, push or break anything. The derived gas never settles or cools.

## 8. Verdict

| Criterion | Score | Why |
|---|---|---|
| Emergence | 3 | An exact ideal gas, effusion, a reversible arrow of time, chaos, clocks and logic, all from one table. But no diffusion in projection, and no matter. |
| Generality | 2 | It runs on any matter without exceptions, but reads one bit of it. Two different rocks are the same mirror. |
| Expressive power | 3 | Universal conservative logic (AND, NOT and fan-out with constants, free crossovers), but no gain, and it needs supply streams and perfect timing. |
| Elegance | 5 | One table, derived rather than chosen. Every block map is an involution, there is one threshold, and the law is exactly reversible bit for bit. |
| Efficiency | 5 | 1 bit per cell, 0.022 ns per cell-tick at rest, 0.11–0.6 ns busy, empty words skipped, and `unstep` at the same cost. |
| Enjoyment | 2 | Rewind and mirrors are delightful, but the world never responds to the particles, and circuits are fragile timing puzzles. |
| Extensibility | 3 | A clean substrate that composes (see below), but one speed and the projection theorem limit what can be added inside it. |

**Strongest objection.** Matter is inert. The law can neither make nor change matter, nor be
changed by it. Worse, the elegant version, which conserves momentum and is isotropic, is
thermodynamically degenerate: every 1-D projection of its gas is exactly free. Restoring diffusion
means breaking momentum conservation and choosing an axis. It is a perfect *transport* layer, not a
universe.

**Suggested composition.** Use margolus particles as the carriers of `demon`'s or `zeldovich`'s
heat and of the game's light. Clear matter already matches the game's transparency. A wall block
could convert wall hits (pressure, measured exactly here) into heat quanta, and hot matter could
emit particles, with the ledger exchanging quanta for particles one for one. That keeps total
energy exact and lets chemistry draw its activation from impacts. Matter would then gain what this
law lacks (change), and particles would gain what the chemistry lacks (reversible, cheap,
ballistic transport and a rewindable record).

## Running

From `physics-lab/` (set `CARGO_TARGET_DIR` to a scratch directory):

```
cargo test --release -p pwc_law_margolus -- --nocapture
cargo run --release -p pwc_law_margolus --example bench
cargo clippy -p pwc_law_margolus --all-targets
```
