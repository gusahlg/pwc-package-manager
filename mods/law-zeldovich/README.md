# Law: Zeldovich (`pwc.law-zeldovich`)

A candidate fundamental law for Project Watt Cubed: chemistry and vibration under **one exactly
conserved integer energy**. Energy is a count of quanta that travel through bonded matter. At a
face, the quanta striking it can carry one occurrence across, paying the binding it is pulled out
of and taking or giving the binding the move changes. A library: it registers nothing.

Every number below comes from a test in `src/tests.rs` (run with
`cargo test --release -p pwc_law_zeldovich -- --nocapture` from `physics-lab/`) or from the bench
(`cargo run --release -p pwc_law_zeldovich --example bench`). Test names are given in brackets.

## 1. Inspiration

- **Yakov Zeldovich and David Frank-Kamenetskii (1938)**, the thermal theory of flame propagation,
  and **Zeldovich (1940)**, the detonation theory later called ZND (with von Neumann 1942 and
  Döring 1943): a detonation is a shock wave sustained by the reaction it ignites, and its strength
  is fixed by the heat of reaction, not by how it was lit. Taken: fuel is a *barrier* plus a
  *release*, and the burning front is the pulse that carries the release to the next barrier. In
  this law a fuse rod is literally a ZND detonation, and its front pulse equals the release.
- **Nikolay Semenov (1928)**, thermal explosion: a reacting body ignites above a critical
  temperature after an induction period that grows without bound as the temperature falls to it.
  Taken: ignition is a threshold on local energy, and the experiments look for its delay.
- **Igor Tamm (1930)** quantised the vibrations of a crystal into countable quanta of sound, which
  **Yakov Frenkel** named *phonons* (1932). Taken: vibration is a *count*, so energy can be an
  exact integer with no rounding anywhere.
- **Andrey Markov's chains (1906)**, and the standard fact about them that a chain whose
  transition matrix is doubly stochastic keeps the uniform distribution stationary. Taken: the
  cell rule is chosen to be doubly stochastic on average, which is what makes heat spread to equal
  energy per bond instead of gathering somewhere.
- **Vyatcheslav Priezzhev, Deepak Dhar, Abhishek Dhar and Supriya Krishnamurthy (1996)**, Eulerian
  walkers (the rotor-router): a deterministic pointer per site that deals walkers round-robin and
  imitates a random walk without dice. Taken: the rotor that deals each cell's uneven remainders.
- **Isaac Asimov, *The Last Question* (1956)**: energy is never lost, yet the usable part of it runs
  down, and the question is whether that can be reversed. Taken: the law conserves energy exactly
  while coherent pulses run down into heat at equal energy per bond, and it says honestly that the
  run-down cannot be undone.
- **Stanisław Lem, *The Invincible* (1964)**: a swarm of tiny identical machines with no designer
  whose collective behaviour is the only intelligence present. Taken: every device here (fuse,
  gate, battery, clock) is a few cells of found matter with no special rule.

## 2. What the universe remembers

| Where | What | Bytes |
|---|---|---|
| cell | the material (a `Mat`, owned by `pwc_law_core::Matter`) | 2 |
| cell | six quanta counts `q[c][d]`: quanta heading for face `d` (`u32`) | 24 |
| cell | the rotor: the last face the cell dealt a remainder quantum to | 1 |
| face | nothing: whether it conducts and what it can do are derived from the two materials | 0 |

`state_bytes_per_cell` is 25. The implementation also keeps 29 bytes per cell of scratch rebuilt
every tick (24 inbound counts, a 1-byte conducting-face mask derived from the bonds, a 4-byte claim
stamp, two activity bits) and per material its binding, smallest barrier and size. A cell with no
quanta does nothing at all and needs to store nothing.

## 3. The law

**Energy.** `UNIT = QUANTUM / 8` raw fit units. The binding of a configuration is
`B = floor(internal_fit / UNIT)`, in quanta. The conserved quantity is

`E = Σ quanta − Σ B − work`,

where `work` is what pokes and kicks added. It is an `i64` sum of integers. Nothing rounds.

One tick is two rules, in this order.

**Face rule (exchange).** For a face between cells `lo` and `hi` along axis `a` (lo = smaller
index), the *impact* is `H = q[lo][+a] + q[hi][−a]`: the quanta heading for that face from both
sides. A candidate move takes one occurrence `e` from a source cell to the other cell (the
destination holds fewer than 32 occurrences, so the void counts as a destination):

- `barrier = max(0, B(src) − B(src∖e))`, the binding `e` is pulled out of;
- `release = B(src∖e) + B(dst+e) − B(src) − B(dst)`, the binding the whole move gains;
- `gain = Σ_{f∈dst} fit_raw(e, f) − holding(e)`, selective transfer's own score.

A move is a chemical event only if `|gain| > N · UNIT` with `N = |src| + |dst|`. That is selective
transfer's threshold (`G > 1/32`), applied in both directions so that every event's reverse is
also an event. An event is affordable if `H ≥ barrier` and `H + release ≥ 0`. The face makes the
affordable event with the highest `gain`, then `lo → hi`, then the smaller element: selective
transfer's tie order. Faces are taken in ascending `(lo, axis)` order. A cell takes part in at most
one move per tick. When a move happens, the impact is spent: `T = H + release` quanta enter the
two cells *through the face*, `floor(T/2)` into `lo` and the rest into `hi`. If a cell is emptied,
its quanta and its share go to the cell that took its last occurrence. A face with no quanta never
reacts, even when a move would cost nothing.

**Cell rule (transport).** Every quantum not spent crosses the face it heads for if that face
*conducts*: both cells are matter and `bond > 0`. It then enters the neighbour through the
opposite face. Otherwise it enters its own cell again through that face (it is turned back). Each
cell then deals the quanta that entered through face `f` over its conducting faces other than `f`,
`floor(n/k)` each. The `n mod k` remainder goes one each to the next faces after the cell's rotor
in the cyclic order −x, −y, −z, +x, +y, +z, and the rotor stops on the last face served. If the
cell has no other conducting face, the quanta go straight on (heading for the face opposite `f`).
Entries are dealt in face order, so the result does not depend on any processing order.

**The gameplay verbs.** `poke(c, s > 0)` deals `s` quanta over the cell's conducting faces (all
six if it has none) and books `s` as work. `poke(c, s < 0)` removes up to `|s|` quanta and books
the amount removed. `kick(c, d, n)` puts `n` quanta heading for face `d`, a directed blow.

**Every constant and where it comes from.** There are no free constants beyond the game's own.

| Constant | Derivation |
|---|---|
| energy unit `UNIT` | `QUANTUM / 8`: the per-occurrence step of selective transfer's threshold |
| binding of a material | `floor(internal_fit / UNIT)` |
| barrier of an occurrence | the binding its block loses without it |
| release of a move | the binding change of the two blocks |
| what counts as an event | selective transfer's threshold `N · QUANTUM / 8`, in both directions |
| which event a face makes | selective transfer's ranking and tie order |
| conduction across a face | `bond > 0` (mean cross-face fit, from `Matter::bond`) |
| capacity | the game's 32 occurrences |
| dealing, rotor, timing | geometry only: no number |

**The ledger is exact by construction.** Transport moves every quantum to exactly one inbound
count, and dealing hands out exactly `n` (`k·floor(n/k) + n mod k`). A move removes `H` quanta,
adds `H + release` quanta and raises `Σ B` by `release`. A poke changes quanta and work by the same
amount. Every channel add is checked, so an overflow would panic rather than wrap.

**Why two rules and not one.** The jackpot would be a single rule: exchange of conserved things
across faces. It cannot be literally one rule, for this reason. A face touches two cells along one
axis, so no face-local rule can change a quantum's axis. With face rules alone, quanta keep their
lines forever and heat never spreads off them. Spreading needs a rule at the cell, where the
axes meet. Among cell rules, the obvious one (go straight, turn only at a wall) routes signals but
is not doubly stochastic. Run on its own, it drives *all* heat into the edges and corners of a
block: interior 0.00, face 0.00, edge 50, corner 150 quanta per cell where 9.38 would be uniform
[`failure_turning_at_walls_would_condense_heat_on_edges`]. Dealing over the other conducting
faces is doubly stochastic, still routes pulses through wires, turns them at corners and splits
them at junctions, and makes heat obey equipartition per bond (section 4). So the minimum is two
rules: a face rule that exchanges energy and matter, and a cell rule that redistributes energy.
Both conserve exactly.

## 4. What emerges

**The ledger closes exactly.** In a closed 8³ box (a 3-D checkerboard fuel bed under a slab of
wire and a void sky, lit at two points), the books are checked after every one of 10,000 ticks.
55,836 moves happen, quanta range from 1,495 to 3,776, binding moves by 2,156, and `E` and the
element count never change. A second universe built the same way reaches the identical digest
[`ledger_closes_exactly_over_ten_thousand_ticks`]. The fire experiments below also assert `E` on
runs that reach 491,565 quanta.

**Cold matter is frozen and free.** Fuel beside fuel with no quanta: 1,000 ticks, 0 active cells,
digest unchanged [`cold_matter_is_frozen_and_costs_nothing`].

**Sound and signals.** In a wire (a cell with two conducting faces) a pulse moves one cell per tick
and keeps every quantum. An L-shaped wire with an 18-cell path delivers all 12 quanta in exactly 18
ticks, because a corner costs nothing. At a T junction 40 quanta split into 20 and 20, arriving at
both ends at tick 10 [`pulses_travel_one_cell_per_tick_turn_corners_and_fan_out`].

**A clock.** A pulse in a bar of length L returns to the identical state every 2L ticks: 10, 32
and 66 ticks for bars of 5, 16 and 33, with 1,000 exact returns each and no decay
[`a_bar_rings_forever_with_period_twice_its_length`].

**Heat obeys equipartition per bond.** 4,800 quanta poked into the centre of an 8³ wire block
reach all 512 cells by tick 16. Averaged over ticks 2001..4000, a cell holds 10.714 (interior,
predicted 10.714), 8.930 (face, 8.929), 7.139 (edge, 7.143) and 5.372 (corner, 5.357) quanta. That
is energy in proportion to its bonds, as the doubly stochastic rule predicts, within 0.3%
[`heat_spreads_to_equal_energy_per_bond`].

**Conduction and insulation.** Two touching 4³ blocks share 3,000 quanta as 1,489 / 1,511 after
3,000 ticks. Separated by a one-cell layer of a material that repels the wire (`bond ≤ 0`), they
end 3,000 / 0, with no quantum crossing and no move happening
[`heat_conducts_across_bonds_and_stops_at_an_insulator`].

**Metastable fuel.** The device fuel pair (found by search) does nothing below an impact of 12. At
12 it moves an occurrence over a barrier of 12 and releases 28
[`a_reaction_happens_only_above_an_impact_threshold`].

**An ignition temperature with an induction delay (Semenov).** A conducting fuel pair (threshold 26)
is buried in a 6³ wire block heated evenly. At 1 to 9 quanta per face nothing happens in 500 ticks.
At 10 per face it ignites after 33 ticks, at 11 after 5, at 12 after 3, and at 13 or more in the
first tick, always at the junction first
[`a_fuel_ignites_only_above_a_temperature_after_a_delay`]. The delay diverges at the threshold, as
in Semenov's theory, from a deterministic rule.

**A flame front that is a pressure wave (ZND detonation).** A 64-cell rod of alternating fuel and
oxidiser, lit at one end by a 12-quanta pulse, burns at exactly 1.000 cell per tick. Its leading
pulse is 28 quanta at every tick from 10 to 48, which equals the release: the fixed point
`H = (H + release)/2` of the face rule. Lit with 12, 24 or 36 quanta, the front settles to the same
28, so the rod restores a logic level whatever drives it. Behind the front, the ash keeps the rest
of the heat, 1,325 quanta at tick 48 [`a_flame_front_is_a_pressure_wave`].

**An amplifier.** In a wire `[W×20] F O [W×60]`, a 12-quanta pulse fires the junction and
`(12 + 28)/2 = 20` quanta come out (gain 1.67), with 20 sent back. The wire is unchanged. Gain is
above 1 only for inputs below the release (`out = (in + release)/2`). The fuse is spent: a second
identical pulse 100 ticks later makes 0 moves and passes 0 quanta
[`an_amplifier_one_small_pulse_releases_a_large_one`].

**A threshold gate, AND and OR.** The same junction reflects 1 and 11 quanta completely, and passes
20, 26 and 32 for inputs of 12, 24 and 36, with exactly one move and an intact wire each time. Two
inputs merged at a T junction (each dealt half onward) drive it. At pulses of 13 it computes AND
(00, 10, 01 → 0; 11 → 21). At pulses of 25 it computes OR (00 → 0; 10, 01 → 21; 11 → 27). The
gate's function is set by the signal level, not by its matter
[`a_threshold_gate_and_and_or_from_one_junction`].

**A battery (energy conversion).** A junction found by search turns a 75-quanta pulse entirely into
binding, holds it for 5,000 ticks, and gives back exactly 75 when struck by a 65-quanta trigger:
140 quanta of pulses, 2.15 times the trigger. Vibration becomes chemistry and returns as vibration
[`a_battery_stores_a_pulse_as_binding_and_returns_it_on_a_trigger`].

## 5. What it cannot do, or does badly

- **No waves.** Quanta carry no phase, so pulses never interfere: two 9-quanta pulses pass through
  each other unchanged [`failure_pulses_never_interfere`]. There is no diffraction, no standing-wave
  node and no refraction. In bulk matter a pulse is dealt over five faces at every cell, so
  sound in bulk is scattered at once. Ballistic sound exists only in wires and rods.
- **One speed for everything.** A pulse crosses ten cells of either of two different wires
  (bindings 1,677 and 1,509) in exactly ten ticks [`failure_one_speed_for_all_matter`]. Stiffness
  and mass do not exist.
- **Not reversible.** Dealing merges histories: one quantum entering a T junction from −x and one
  entering from +y leave identical states, so `unstep` returns false
  [`failure_dealing_is_not_reversible`]. The law conserves energy exactly but is not
  time-reversible. Its arrow of time is the run-down of pulses into equipartitioned heat.
- **Fire burns the world.** Binding is a pair sum, so it grows like n² with the occurrences in a
  cell and never saturates. Clumping releases energy without bound once an asymmetry seeds it. Pure
  wire is immune: its only moves between equal cells have zero gain, so 10,000 quanta per face
  make 0 moves. But fuel atoms pushed into hot wire seed a runaway. For fuel beds of 1 to 5 layers
  under wire in an 8³ box, lit with 600 quanta, the quanta after 1,000 ticks were 2,374, 23,690,
  2,756, 491,565 and 454,152. The wire cells changed were 2/448, 15/384, 0/320, 256/256 and 192/192.
  There is no clean critical size: the outcome depends on the geometry
  [`failure_a_big_fire_burns_the_wire_too`].
- **Devices are fragile and one-shot.** A conducting junction is chemically reactive by nature,
  because `bond > 0` means its elements attract across the face. Only barriers protect it. The
  device fuel had to be searched for robustness over a 3× amplitude window. Fuel is spent after
  one firing, and nothing re-arms it.
- **No NOT.** Counts are never negative, so a pulse can only add. Inhibition would need chemistry
  that blocks a path, which is again one-shot.
- **Repelling interfaces are never struck by diffuse heat.** Dealing only sends quanta to
  conducting faces. Non-bonded faces react only to pulses that arrive straight on (wire ends,
  isolated cells), never to bulk heat.
- **Where exactness had to give.** (1) Energy is exact in quanta, but a fit difference smaller
  than `UNIT` is not energy: binding is floored, so a move's release can differ by one from
  `gain / UNIT`. (2) Reversibility, as above. (3) Momentum: headings route quanta but are not
  conserved, so "pressure" is an energy pulse, not a momentum flux. (4) Which event a face makes is
  selective transfer's ranking: deterministic, but an imported choice rather than a consequence of
  energy.

## 6. Cost

Bench: a 64³ box (262,144 cells, 50% matter: rock, soil and eight buried 6×3×6 fuel pockets),
single thread, Intel i5-12400F.

| Case | ns per cell-tick (whole box) | active cells | ns per active cell-tick | moves per tick |
|---|---|---|---|---|
| at rest | 0.037 | 0 | – | 0 |
| heat, 64 surface pokes of 600, ticks 1..100 | 3.05 | 17,570 (6.7%) | 45.6 | 0 |
| heat, ticks 101..600 | 4.73 | 27,718 (10.6%) | 44.8 | 0 |
| fire, the eight pockets lit, ticks 1..100 | 3.71 | 15,419 (5.9%) | 63.1 | 1.4 |
| fire, ticks 101..400 (out) | 5.02 | 22,883 (8.7%) | 57.5 | 0 |
| inferno (exposed 64×4×64 fuel bed), ticks 1..25 | 1.41 | 346 | 1,070 | 77.7 |
| inferno, ticks 26..50 | 76.0 | 4,743 | 4,202 | 815.7 |
| inferno, ticks 51..75 | 1,012 | 18,092 | 14,660 | 2,584.8 |
| inferno, ticks 76..100 | 5,638 | 40,600 | 36,400 | 5,138.0 |

- **Sparsity.** Work is proportional to the cells holding quanta. At rest the cost is a scan of
  4,096 activity words per tick. The buried pockets fizzle, because conducting rock drains their
  heat. The ledger stays exact in every case (the bench asserts it).
- **Memory.** 25 bytes of state, 29 bytes of scratch and 2 bytes of material per cell. A world host
  would store quanta only for active cells: a cold cell's state is all zeros.
- **The inferno is dominated by law-core, not by the law.** A burning world creates new
  configurations: the palette grows from 250 to 12,817 in 100 ticks. `perf` puts **94.45%** of the
  whole bench in `pwc_law_core::matter::bond_of`, which `Matter::intern` calls against every palette
  entry for each new configuration, to fill a dense P² bond table. This law only reads the bonds of
  the six neighbours of a changed cell. **law-core gap:** a lazily computed, cached
  `bond(a, b)` would remove this cost. A dense P² table (657 MB at P = 12,817) cannot work at world
  scale for any chemistry-making law. I did not change law-core.
- **What a world-scale host needs.** Chunked activity bitsets. Claims and moves are face-local, so
  chunks can step in parallel, provided border faces are taken in the global `(lo, axis)` order.
  Integer determinism makes lockstep or server-authoritative play possible. Network deltas are the
  moves (face, element) plus the quanta of active cells. Saves hold the quanta and rotors of active
  cells only. The palette needs lazy bonds (above) and garbage collection of dead configurations.

## 7. Gameplay

- **Verbs.** `poke` heats a cell (dealt over its conducting faces) and `kick` strikes it in one
  direction, a hammer blow that sends a pulse down a wire. Negative `poke` cools.
- **What a player would discover and build.** Wires that carry a blow around corners at one cell per
  tick and split it at junctions. Insulators that keep a forge hot. Fuses and detonating cord whose
  flame runs at the speed of sound with the same strength whatever lit it. Amplifiers, threshold
  gates and AND/OR gates set by the pulse strength. Batteries that swallow a blow and return it
  on a trigger. Bars that ring forever as clocks. Ignition points with a suspenseful delay just
  above them. All of this is found in matter by experiment, with no recipe.
- **What would confuse or frustrate.** A big fire can burn ordinary terrain into dense clumps,
  which is spectacular but destructive. Devices work only within an amplitude window and burn once.
  Pulses never cancel, so there is no NOT gate to build. All matter carries sound at the same
  speed. Cracks between repelling materials never react to heat.

## 8. Verdict

| Criterion | Score | Why |
|---|---|---|
| Emergence | 4 | Detonation, ignition delay, equipartition, gates and batteries all emerge unauthored, but there are no waves and no chemical oscillator |
| Generality | 4 | Every constant comes from fit and any matter runs, but useful devices needed searches over many pairs to find robust ones |
| Expressive power | 3 | Clock, threshold, AND/OR, amplifier, battery and level restoration, but no NOT and no re-arming fuel |
| Elegance | 3 | One exact integer energy and two local rules, but the face rule imports selective transfer's search, threshold and tie order |
| Efficiency | 4 | A cold world is free and an active cell-tick costs about 45 ns, but chemistry-heavy scenes are crushed by palette growth |
| Enjoyment | 4 | Fire, fuses, hammers, insulation and batteries are tangible, but world-burning fires and fragile devices frustrate |
| Extensibility | 4 | New matter just works, the state is sparse and the rules are local; reversibility could not be added later |

**The strongest objection.** It is two rules and an import, not one rule. Mechanics and chemistry
share an exact energy but not a mechanism. Vibration is a deterministic random walk of energy
packets (a rotor-router diffusion with ballistic wires), with no momentum, no phase and one speed,
so "sound" is energy hopping rather than a wave. Chemistry is still selective transfer's move
search, gated by energy instead of driven by it. And because binding grows like n² without
saturation, the energy that makes fire possible makes every hot world clump.

## Licence

`Apache-2.0 OR MIT`, at your option.
