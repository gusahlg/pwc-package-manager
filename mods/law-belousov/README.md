# Law: Belousov (`pwc.law-belousov`)

A candidate fundamental law for Project Watt Cubed: **the excitable medium**. Every matter cell
remembers one phase. A resting cell fires when its glow, raised by the bonds to its excited
neighbours, crosses the line where the game's matter starts to shine. A fired cell then recovers
through a refractory period whose length is set by its cohesion. Positive bonds excite, negative
bonds inhibit, and blocks that already shine are clocks. Nothing else exists.

This is escape 2 of the brief: the law keeps dissipation (nothing is conserved and every
excitation decays) but has a free **drive**, the recovery of every cell to rest. A library: it
registers nothing.

Every number below comes from `src/tests.rs` (run with `--nocapture`) or from the bench
`physics-lab/laws/belousov/examples/bench.rs`, and the test that produces it is named.

## 1. Inspiration

- **Boris Belousov** (1951, published only as a 1959 abstract after rejection): a chemical
  reaction that oscillates while it runs downhill. *Taken:* dissipation does not forbid clocks,
  provided something is fed in. Here the feed is the cells' free recovery.
- **Anatol Zhabotinsky** (from 1961), and **Zaikin and Zhabotinsky** (1970): the same reaction in
  a thin layer carries target waves and spirals. *Taken:* the experiments to aim at, namely
  waves that annihilate head-on, targets around a pacemaker, and spirals around a broken wave
  end.
- **Wiener and Rosenblueth** (1946), **Gelfand and Tsetlin's** Moscow work on models of excitable
  tissue (around 1960), and **Balakhovsky** (1965), who drew the first spiral in a discrete
  excitable medium. *Taken:* the three-part cell (rest, excited, refractory) and a discrete tick.
  **Greenberg and Hastings** (1978) later gave the cellular-automaton form that this law
  generalises. In their medium, persistent activity is tied to a phase defect, and the
  experiments measure that defect as a winding number.
- **Valentin Krinsky** (1966): in heart tissue whose patches recover at different rates, a wave
  train breaks up into re-entrant spirals, which is fibrillation. *Taken:* an experiment that
  tests whether this happens here. It does **not** (section 5).
- **Ilya Prigogine** (dissipative structures, 1960s–70s): order is sustained far from equilibrium
  by a flow of free energy. *Taken:* the requirement to say exactly what the drive is (section 3,
  "Energy").
- **Science fiction.** Arkady and Boris Strugatsky, *Roadside Picnic* (1972): the Zone's
  *etaks*, batteries that never run down. A pacemaker here is one of those, a block that beats
  forever for free. That is the law's charm and also its deepest objection. Stanisław Lem,
  *Solaris* (1961): an ocean whose waves are its thought. The wish here is the same, a medium in
  which computation is weather.

## 2. What the universe remembers

- **Per cell: one byte, the phase `φ`.** 0 means rest, 1 means excited (for exactly one tick) and
  `2..P` means refractory. The largest period any configuration reaches is 7 (cohesion is at most
  1152, which gives `3 + 1152/256 = 7`), so `φ < 8`, and bit 7 serves as a scratch mark during a
  tick.
- **Per face: nothing.** A face's coupling is the bond between the two materials, which the
  matter already holds.
- **Per material** (derived once from readings): `drive` (i32) and `P` (u8).
- **Working lists** (not state): the live cells (`φ ≠ 0`), the excited cells, the pacemakers, and
  scratch for the candidates and firings. All of them are O(activity) `u32` lists that are reused
  every tick, so no tick allocates once the lists reach their working size.

`state_bytes_per_cell() = 1`. A quiet world's state is all zeros.

## 3. The law

One tick, synchronous (every decision reads the phases from before the tick):

```
resting cell c (φ = 0, not void):
    fires  ⇔  drive(c) + Σ_{faces n with φ_n = 1} bond(c, n)  >  0        → φ := 1
otherwise (φ ≥ 1):
    φ := φ + 1, and φ := 0 when it reaches P(c)
```

**Constants, every one derived from readings (Q8 of `QUANTUM`):**

| Constant | Formula | Meaning |
|---|---|---|
| `drive(m)` | `glow(m) − GLOW_FROM`, with `GLOW_FROM = 448` | how far the material sits above the firing line |
| `P(m)` | `3 + max(cohesion(m), 0) / 256` | 1 excited tick, then `1 + cohesion/QUANTUM` refractory ticks |
| coupling | `bond(m, n)` | the mean cross-face fit: positive excites, negative inhibits |

`GLOW_FROM = 7·64` is the game's own emission onset (`material::observe`): a block emits light
iff `glow > 448`. Writing the firing line there means that **the light-emitting blocks are
exactly the pacemakers**. This is checked on 20 000 random configurations
(`pacemakers_are_exactly_the_matter_that_shines`). An excitation is a neighbour lending you its
bond's worth of glow for one tick, and a firing is a cell crossing the line where matter shines.
A renderer could show an excited cell as a one-tick flash of emission. No other constant exists.

**Deviations from the brief's spec, and why:**

- **`X = 1` for all matter (the spec suggested deriving it).** A firing is an event that lasts one
  tick. A longer excited span needs another constant with no natural reading, and it blurs
  coincidence: with `X = 1` the AND, the NOT and the half adder have exact truth tables. The cost
  is that gates need exact timing (section 5).
- **The threshold is folded into `drive`.** "Fires when the sum reaches its threshold" with
  `threshold = GLOW_FROM − glow` is the same rule. Writing it as "glow + input crosses the shining
  line" removes the special case: a pacemaker is simply matter whose drive is already positive.

**Update order:** synchronous. In code, (1) every resting matter neighbour of an excited cell,
plus every resting pacemaker, is collected once (marked in its phase byte), (2) each candidate is
decided from the old phases, and (3) every live cell advances, after which the fired cells become
`φ = 1`. The result does not depend on list order. `the_sparse_law_equals_the_dense_rule` checks
it against a dense, double-buffered rewrite every tick for 500 ticks with 1500 random pokes on a
mixed 960-cell box, and two peers stay digest-equal.

**`poke(i, s)`:** `s > 0` shocks the cell (`φ := 1`), `s < 0` quenches it into its first
refractory tick (`φ := 2`) and `s = 0` does nothing. Void ignores pokes. **`unstep`:** not
available, because the law is many-to-one (a cell forgets how it came to rest).

**Energy.** Nothing is conserved. The drive is the free return of every cell to rest: each
firing would, in a chemical medium, consume reagent, and here it costs nothing. A lone pacemaker
fires 25 000 times in 100 000 ticks (`nothing_is_conserved_a_pacemaker_fires_forever`). The law
counts the bill (`ledger().fired`). If gameplay should make firing consume something (Prigogine's
condition for an honest dissipative structure), the clean composition is with `demon`: a firing
costs one heat quantum from the cell, and a cell without heat cannot fire. Clocks would then
need fuel and spirals would burn out. That is not implemented here.

## 4. What emerges

The experiments run on materials found by deterministic search (seeds 101–109 of
`pwc_law_core::search::find`, 2- or 3-kind mixtures of 16 occurrences). Every inequality a circuit
relies on holds by at least 32 (1/8 `QUANTUM`), and every period-setting cohesion lies at least
32 from a step, so no result rests on a knife edge:

| | glow | drive | cohesion | P | self-bond | bond to `w` | role |
|---|---|---|---|---|---|---|---|
| `w` | 408 | −40 | 428 | 4 | 401 | 401 | wire: excites itself, never fires alone |
| `j` | −283 | −731 | −45 | 3 | −42 | 462 | junction: needs two excited `w` |
| `v` | 390 | −58 | 414 | 4 | 388 | **−486** | second wire, inhibits `w` |
| `g` | 515 | 67 | 447 | 4 | 419 | 89 | pacemaker |
| `fast` | 808 | 360 | 37 | 3 | 34 | 330 | pacemaker, faster than `w` recovers |
| `slow` | 512 | 64 | 550 | 5 | 516 | 192 | pacemaker |
| `lag` | 167 | −281 | 548 | 5 | 514 | 371 | wire that recovers one tick slower than `w` |
| `rock` | −66 | −514 | 353 | 4 | 331 | −202 | ordinary cohesive matter |

| Phenomenon | Measurement | Test |
|---|---|---|
| Signal speed | exactly 1 cell/tick in both `w` and `v`; a pulse crosses 400 cells undiminished; shocked every tick, the far end fires every 4 ticks (= P) | `a_pulse_runs_at_one_cell_per_tick_in_every_wire` |
| One-way propagation | a shock in the middle of a 41-cell wire sends two pulses out, and each cell fires exactly once | `refractoriness_makes_pulses_one_way_and_collisions_annihilate` |
| Annihilation | two pulses meet at cell 20 at tick 20 and vanish; 39 firings, then the wire is at rest and costs 0 | same |
| Diode | a 2×2 corner: the input splits into two cells that reach a `j` junction together. Forward, `j` fires at tick 7 and the output at tick 12. Backward, `j` gets one input and stays dark. The same shape built of `w` conducts both ways | `a_two_by_two_junction_is_a_diode` |
| Pacemaker clock | `g` beats every 4, `slow` every 5 and `fast` every 3 ticks, each exactly its `P` from cohesion. A clock faster than the wire is **divided**: `fast` (3) drives `w` (4) at one pulse per 6 ticks | `a_pacemaker_ticks_with_the_period_its_cohesion_sets` |
| AND, OR | the same two-input picture with `j` or with `w` at the junction gives the AND and OR truth tables | `and_or_not_from_bonds_alone` |
| NOT (clocked) | a `g` clock line passes a beat every 4 ticks, and a `v` pulse arriving beside the gate cell in phase suppresses exactly that beat (tick 24). One tick early or late does nothing | same |
| Half adder | each input forks to an OR cell and an AND junction; the junction fires the inhibiting `v` wire, which meets the OR output at the sum gate in the same tick. Results: `00→S0 C0`, `10→S1 C0` (S at tick 20), `01→S1 C0`, `11→S0 C1` (C at tick 17) | `a_half_adder_from_three_wire_materials` |
| Memory | a 36-cell ring holds 2 pulses for 10 000 ticks; the state recurs exactly every 18 ticks (36 per lap); capacity is 36 / P = 9 pulses | `a_ring_remembers_its_pulses_forever` |
| Spiral wave | a wave broken by a quench curls into a spiral on a 64×64 sheet that still turns after 5 000 ticks. The whole sheet recurs exactly every 4 ticks, and the phase winds **+1** round every square of radius 4 to 30 about the tip and 0 round squares away from it | `a_broken_wave_curls_into_a_spiral_that_never_stops` |
| Spiral made by matter | two plain shocks (no quench): a line across a `w` sheet, and a line one row ahead across a half sheet of `v` underneath. The `v` wave leads by one tick and kills the `w` wave over that half. The free end becomes a spiral, with winding −1 at radius 6, 12 and 18 after 3 000 ticks | `an_inhibitory_layer_breaks_a_wave_into_a_spiral` |
| Fan-out (gain) | one shock reaches all 16 branch ends of a comb, each exactly on time; 367 firings from 1 shock, and the loop-free tree returns to rest | `fan_out_regenerates_every_branch` |
| Determinism | sparse equals dense every tick, and peers are digest-equal | `the_sparse_law_equals_the_dense_rule` |

The half adder as drawn (`a`, `b` inputs; `s` the sum gate; `c` the carry tap on the `v` wire)
and at tick 19 with both inputs set, when the inhibiting pulse (`@` right of the gap) meets the
OR output (`@` left of it):

```
..........a..........
..........w..........
..........w..........
...wwwwwwwwwwwwwww...                   ···············
...w.............w...                   ·             ·
...w.............w...                   ·             ·
...wwwwwwwsvvcvvvj...                   ····:-@·@-:····
...w.............w...                   ·             ·
...w.............w...                   ·             ·
...wwwwwwwwwwwwwww...                   ···············
..........w..........
..........w..........
..........b..........
```

The gates are **restoring**: firing is all-or-none and every cell refires at full strength, so
signals never attenuate and fan-out is free. Together with clocked NOT, that is a complete,
restoring logic family made of nothing but bonds: gain, which the selective-transfer law cannot
have at all.

## 5. What it cannot do (or does badly)

- **Spirals never nucleate on their own.** Krinsky's mechanism (a wave train breaking on a patch
  that recovers more slowly) does not happen here. In 64 runs (8 patch shapes, trains of 2, 3, 5
  and 10 waves, and a slow conducting `lag` patch or an inert `rock` obstacle), the waves wrap
  round, close behind and the sheet returns to rest (`wave_trains_never_nucleate_spirals_on_their_own`).
  A spiral needs a wave already broken, either by a quench or by an inhibitory pulse timed
  exactly, which matches the Greenberg–Hastings picture in which sustained activity needs a phase
  defect. The world never becomes turbulent by itself.
- **Signal speed is the same everywhere.** Exactly 1 cell/tick in every conductor
  (`a_pulse_runs_at_one_cell_per_tick_in_every_wire`). There is no refraction, and delay comes only
  from path length.
- **Exact timing.** With `X = 1`, inhibition works only in the very tick of arrival. One tick
  early or late is ignored (`and_or_not_from_bonds_alone`). Gates need path lengths equal to the
  cell, and the half adder is a careful drawing, not something that would form by itself.
- **Clock periods are short and few.** `P = 3..7` in principle, but the searches found only 3, 4
  and 5 (cohesion up to about 550; an even pair of 16 occurrences is bounded by 614). Slower
  clocks need rings (period = ring length) or dividers.
- **Most matter is inert.** In a census of 20 000 random configurations (1–4 kinds, 16
  occurrences), 10.4% are pacemakers, 5.0% conduct along themselves and 84.5% are inert. A pure
  block never conducts (identical elements fit 0, so its self-bond is 0), and neither does
  ordinary rock (`pure_and_ordinary_matter_stays_dark`). Conduction is a property of special
  mixtures, which is good for gameplay, but most of the world does nothing.
- **Matter never changes.** The law moves information, never elements. There is no chemistry, no
  mechanics, no transport of anything material. It is a signal layer, not a universe. It needs a
  host law (selective transfer, `demon`) underneath, with tables rebuilt when matter changes.
- **Nothing is conserved and the drive is free** (`nothing_is_conserved_a_pacemaker_fires_forever`):
  pacemakers are perpetual motion. Section 3 has a proposed fuel rule.
- **Not reversible.** `unstep` returns false.

## 6. Cost

Bench (`cargo run --release -p pwc_law_belousov --example bench`): a 64³ box (262 144 cells, 50%
filled) holding a terrain slab of rock (y < 20), a conducting `w` layer (20 ≤ y < 28) and soil
(28 ≤ y < 32), on one core (repeat runs vary by about 5%):

| Case | ns / cell-tick | cells / s | active / tick | ns / active cell | µs / tick |
|---|---|---|---|---|---|
| at rest | 0.000 | 5.1e13 | 0 | – | 0.0 |
| at rest with 64 pacemaker lamps | 0.010 | 9.9e10 | 191 | 13.9 | 2.7 |
| scroll wave in the layer | 1.607 | 6.2e8 | 34 816 (13.3%) | 12.1 | 421 |
| scroll wave filling the whole slab | 6.132 | 1.6e8 | 131 072 (50%) | 12.3 | 1 608 |

(The "cells / s" at rest is the box size divided by an empty tick; the honest figure is the
0.0 µs per tick.)

- **Sparsity:** the cost is proportional to activity, about 12 ns per active cell, and **zero at
  rest**. Pacemakers are the exception: each costs about 40 ns per tick forever.
- **Memory:** 1 byte per cell, plus 4 bytes per active cell in the lists.
- **The catch:** a spiral or scroll wave keeps its **whole connected conductor** active forever
  (4096 of 4096 sheet cells per tick in the spiral test). A large conducting region with one
  defect in it is a permanent full-load.
- **A world-scale host would need:**
  - per-chunk live, excited and pacemaker lists;
  - a two-phase tick at chunk borders (synchronous update reads neighbours' old phases);
  - per-material `drive` and `P` columns in the registry hot tables;
  - rebuilding a chunk's pacemaker list when matter changes.
  - **Saves:** only cells with `φ ≠ 0`, which is usually nothing.
  - **Network:** nothing beyond the pokes, since the law is integer, synchronous and digest-checked,
    so peers in lockstep stay equal (or the server sends the fired-cell set per tick).
  - **Pacemakers:** an isolated pacemaker (no neighbour it can excite) affects nothing but its
    own flash, so the host can compute its phase from the tick number instead of visiting it.
  - **Unloaded chunks:** waves that reach unloaded chunks stop, so circuits must live in loaded
    ground.

## 7. Gameplay

- **What a player discovers:** glowing blocks *beat*, flashing every 3 to 5 ticks. Certain
  mixtures carry a flash from a beating block across the ground like a nerve. Some pairs of
  mixtures **silence** each other where they touch. A shock (`poke` > 0) sends a pulse down a
  wire, and a quench (`poke` < 0) kills one or breaks a wavefront.
- **What a player builds:** clocks and frequency dividers, delay lines (meanders), diodes
  (a 2×2 corner of junction material), AND/OR/NOT, adders, ring memories that hold pulses
  indefinitely, alarms (a tripwire wire to a lamp), and, by breaking a wave in a conducting
  field, a **spiral**: a perpetual beacon that sweeps a whole region. It is redstone with real
  chemistry under it: what conducts, what inhibits and what beats is not a list of items but a
  consequence of what the blocks are made of, and it is found by experiment.
- **The poke verb:** the player's touch is a shock or a quench, one cell, one tick.
- **What would confuse or frustrate:**
  - Gates work only with exact path lengths, and an input one tick off is silently ignored.
  - Most blocks do nothing.
  - Signals are invisible unless the renderer flashes excited cells, which the law invites, since
    excitation *is* crossing the shining line.
  - A careless break in a conducting field makes a spiral that never stops and keeps the region
    busy.
  - Free perpetual clocks undercut any economy unless firing costs fuel.

## 8. Verdict

| Criterion | Score | Why |
|---|---|---|
| Emergence | 4 | Waves, annihilation, spirals (winding ±1), clocks, frequency division, memory, gain and logic all come from one rule; but spirals never nucleate unprompted and matter never changes |
| Generality | 3 | Every material gets its constants by formula with no exceptions, but 84.5% of random matter is inert, pure blocks never conduct, and the speed is the same everywhere |
| Expressive power | 4 | A complete, restoring, clocked logic family plus memory and a half adder, built from bonds alone; held back by exact-tick timing and periods limited to 3–5 |
| Elegance | 5 | One byte, one inequality, two readings; a pacemaker is no special case, inhibition is no extra rule, and the firing line is the game's own emission onset |
| Efficiency | 4 | Free at rest, 1 byte per cell, about 12 ns per active cell; but a spiral keeps its whole conductor busy and the list-driven visit has poor locality under full load |
| Enjoyment | 3 | Discoverable circuits and living patterns are fun, but exact timing and invisibility frustrate, and free clocks break an economy |
| Extensibility | 4 | A thin layer that composes with any matter law (rebuild tables when matter changes) and with `demon` for a fuel rule; on its own it cannot move or change matter |

**Strongest objection.** This is not a physics of the world. It is a nervous system laid over
static matter. Nothing moves, nothing is conserved, and every pacemaker is a perpetual motion
machine, so as a *fundamental* law it explains none of the material change the game is about. Its
logic and spirals exist only because the drive is free. To be a universe it must be composed with
a law that changes matter and with an energy ledger that makes firing cost something. Beyond that
it is a beautiful signal layer.

## Running

From `physics-lab/` with `CARGO_TARGET_DIR` set to a scratch directory:

```
cargo test --release -p pwc_law_belousov -- --nocapture
cargo run --release -p pwc_law_belousov --example bench
cargo clippy -p pwc_law_belousov --all-targets
```

## Licence

`Apache-2.0 OR MIT`, at your option.
