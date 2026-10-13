# Law: Demon (`pwc.law-demon`)

The microcanonical candidate law. It keeps selective transfer's chemistry and adds one conserved
quantity: **heat**. Every matter cell holds a count of quanta. A contact makes the best
selective-transfer move that each of its two cells can pay for from its own heat. Heat conducts
across bonded faces. The total `Σ heat − Σ binding − work` is conserved exactly, tick by tick.

Every number below comes from `src/tests.rs` (run with `--nocapture`) or from the bench in
`physics-lab/laws/demon/examples/bench.rs`.

## 1. Inspiration

- **Landau and Lifshitz, *Statistical Physics* (1938; the 1951 edition of the Course).** The
  starting point is the microcanonical ensemble: an isolated system has a fixed energy, and
  temperature is only a way to describe how that energy is shared. From it the law takes an
  exact energy ledger and no thermostat. Temperature is not a parameter. It is heat per cell.
- **Semenov, the theory of thermal explosion (1928; *Chemical Kinetics and Chain Reactions*,
  1934).** Ignition happens when heat release outruns heat loss. From it comes the preheating
  experiment: a front speeds up as the medium warms, and the whole charge goes at once once it
  passes its ignition temperature. The 3-D fizzle in the bench is the same balance in the other
  direction.
- **Zeldovich and Frank-Kamenetskii, thermal flame propagation (1938); Zeldovich, Barenblatt,
  Librovich and Makhviladze, *The Mathematical Theory of Combustion and Explosions* (1980).**
  Fuel is metastable: it is energetically downhill but kinetically locked behind an activation
  barrier. A front is conduction feeding activation. From this come the barrier and the
  combustion front.
- **Kolmogorov, Petrovsky and Piskunov (1937).** A travelling front has a speed set by the
  medium. The front experiment measures that speed.
- **Creutz, the microcanonical "demon" (1983) and deterministic Ising dynamics (1986).** This is
  not Soviet work, but it gives the mechanism: each site has a demon that pays for or absorbs
  energy changes, and the updates are deterministic. Here every cell's heat is its demon.
- **Stanisław Lem, *The Cyberiad* (1965), "The Sixth Sally"**, with its Demon of the Second Kind,
  which sorts molecules and pulls information out of them. That story is where the name comes
  from, and it states the warning in section 8: a demon that sorts matter can be asked for an
  unbounded amount of it.
- **Aleksey Tolstoy, *The Garin Death Ray* (*Hyperboloid of Engineer Garin*, 1927).** This
  shaped the gameplay verb: concentrated heat put into one place. `poke` is a hyperboloid, and
  a negative `poke` is its opposite.

## 2. What the universe remembers

- **Per cell:** the material index (law-core's `Matter`) and `heat: u32`, so
  `state_bytes_per_cell = 4`. Void never holds heat, because heat lives in matter.
- **Per face:** nothing.
- **Global:** the tick phase (`tick mod 6`), which is included in `digest`.
- **Derived caches that are not state:** binding per palette entry, and a memo of each ordered
  material pair's moves (with results interned lazily). Per-tick scratch is one flag byte and
  one `i64` conduction sum per cell, plus the sorted list of warm cells.

## 3. The law

One tick has two parts, in this order.

**Conduction (synchronous).** Each face between two matter cells `a`, `b` carries
`f = trunc(max(bond(a,b), 0) · (q_a − q_b) / 2^13)`. All fluxes are computed from the old heat
and applied together. They are antisymmetric, so `Σ q` is conserved exactly.

**Chemistry (one dimer phase).** Axis `t mod 3` and parity `(t/3) mod 2` pair each cell with at
most one face neighbour, so contacts are disjoint and their order cannot matter. Contacts are
still run in cell-index order, so new configurations get the same palette indices on every
peer. For a contact (lower cell A, upper cell B), every selective-transfer move is a candidate:
a transfer of one occurrence either way into a block below capacity, and swaps only when a block
is at capacity. Each candidate changes each cell's binding:

- transfer of `e` from S to D: `δ_S = bind(Φ_S − h_S(e)) − bind(Φ_S)`,
  `δ_D = bind(Φ_D + p_D(e)) − bind(Φ_D)`;
- swap `e ∈ A`, `f ∈ B`: `Φ_A' = Φ_A − h_A(e) + p_A(f) − J(e,f)`, and the same for B.

Here `Φ` is `Block::internal_fit`, `h` is `Block::holding`, `p_D(e) = Σ_{d∈D} fit_raw(e, d)` and
`J = fit_raw`. A move **sets each cell's heat to `q + δ`**. It is affordable when both results
are `≥ 0` and a giving cell keeps at least one quantum (`q + δ ≥ 1`: no heat, no motion). Of the
affordable moves, the contact makes the one with the **highest fit gain**, which is the exact
change of `Σ Φ`, with selective transfer's tie order. A cell that is emptied passes its heat on
with its last occurrence.

That is the whole law. Two consequences follow without any extra rule:

- **Chemistry conserves each cell's energy** `q − binding` exactly. Energy moves only by
  conduction, and with matter that moves into void. Asserted cell by cell in
  `a_lock_flicker_keeps_every_cell_energy_exact`.
- **Activation is not a separate rule.** Pulling `e` out costs the giving cell its own binding
  loss, which is `floor(h/UNIT)` up to flooring. The receiving cell is paid the attraction
  `p`. Fuel is therefore locked until its own cell holds its holding plus one quantum.

**Every constant and where it comes from:**

| Constant | Value | Derivation |
|---|---|---|
| `UNIT` (one quantum) | `QUANTUM / 256` | the Q8 scale every `Reading` uses, so heat and readings share units |
| `binding(c)` | `floor(internal_fit / UNIT)` | `Block::internal_fit` (Σ pair fit) |
| conductance | `max(bond, 0)` | `Matter::bond`. Void and repelling faces insulate. A pure block bonds 0 to itself, so it never conducts. |
| `CONDUCT_SHIFT` | 13 | smallest `s` with `6 · max bond ≤ 2^s`. Every bond is below 1152, the most `fit_raw` can give. This keeps the maximum principle, so heat never goes negative. |
| barrier | `≈ floor(h/UNIT) + 1` in the giving cell (exactly `−δ_S + 1`) | `Block::holding`, through the giver's `δ` |
| release into the receiver | `δ_D ≈ p_D(e) / UNIT` | `fit_raw` attraction |
| `HEAT_CAP` | `2^28` per poke | an overflow guard, not physics |

No material is named, and the memo is the only per-pair table. It is derived from fit and built
on demand.

**Where this deviates from the brief, and why.** The brief pools the contact's heat (`H = q_A +
q_B`), gates a move on `H > barrier`, and returns the barrier on arrival. That version was built
and measured first, with the heat after a move split evenly between the two cells. Every reaction
then equalised the heat of its two cells, which made each move an instant conductor. The
consequence was that every fuel tried burned at exactly the causal limit of 1 column per 3 ticks,
whatever its chemistry (5 fuels checked, ignition ticks 1, 6, 9, 12, …). That was measured during
development on the discarded variant, and no test keeps it. The per-cell rule is
more local: one cell pays and one is paid. It needs no barrier rule and no split rule, and its
front runs on conduction at about a sixth of the causal limit (section 4). The choice "highest fit gain" uses the exact
`Σ Φ` change rather than the floored binding change. The two agree except within one quantum of
flooring, and the fit gain keeps selective transfer's ordering and ties.

## 4. What emerges

The materials come from deterministic searches whose predicates are in `src/tests.rs`:

- a fuel F: a cohesive 8-occurrence mixture;
- an oxidizer X whose best move with F releases between 1 and 2 times what it needs;
- a sink S: cohesive, conductive, and inert below 3000 quanta;
- a firebreak: a pure block that repels every element of F, X and S;
- an ore and a collector;
- the two halves of a lock.

| Phenomenon | Measurement | Test |
|---|---|---|
| Absolute rest when cold | 600 ticks, 0 active cells, identical digest | `cold_matter_never_moves` |
| **Exact ledger** | `E` identical after every one of 6000 ticks, with 12 pokes (heat in and heat out) and 81 moves in a 12×6×3 box; occurrences identical | `the_ledger_closes_exactly_over_thousands_of_ticks` |
| Exact *local* conservation | each cell's `q − binding` identical through 102 moves | `a_lock_flicker_keeps_every_cell_energy_exact` |
| **Ignition temperature** | fuel holdings + 1 are 377 and 628 quanta. At uniform T = 0, 1, 188 or 376: no move in 1200 ticks. At T = 377: moves. At T = 628 the best move fires and binding rises by up to 6901 quanta (11× its ignition). | `fuel_ignites_at_a_sharp_temperature_set_by_its_holding` |
| **Combustion front** | 48-column rod on a sink, sparked at one end. It crosses in 871 ticks. Speed 0.0463 columns/tick over columns 8–28, then 0.0745 over 28–47, 0.0568 overall. That is about 6× slower than the causal limit, and conduction-fed. | `a_combustion_front_crosses_the_rod_at_a_steady_speed` |
| **Thermal runaway / explosion** | 32-column rod, ticks to burn all of it after preheating to 0, ¼, ½ and ¾ of ignition: 824, 680, 457, 290. Preheated to ignition: all 32 columns in 1 tick. | `preheating_speeds_the_front_and_past_ignition_it_explodes` |
| **Insulation / firebreak** | one column of a repelling pure block: the 20 near columns burn, and exactly 0 quanta reach the far side in 1500 ticks | `a_firebreak_stops_the_front_and_no_heat_crosses_it` |
| **Smelting** (heat drives what cold forbids) | every ore/collector move is uphill. Ore heat ≤ 2750: nothing. At ≥ 3000: one occurrence separates and binding falls by 2189 (heat is consumed). After a quench (heat poked out), the products persist for 600 ticks. | `heat_drives_a_separation_that_cold_forbids_and_a_quench_keeps_it` |
| **The law never halts** | closed 3-cell box X F X. With 2000 or 4000 quanta put in, it enters a cycle after 577 or 511 ticks with **period 90** and 28 moves per period. With 9000 quanta it settles into a period-12 flicker. A fixed point would have period 6. | `a_closed_box_never_halts_it_settles_into_a_cycle` |
| **Relaxation oscillator** | in that period-90 cycle the fuel cell charges by conduction (0 → 59 → 116 …) and fires when it can pay, dropping back to 0. The period is set by conductance and holdings, not by the schedule. | same |
| **Thermal AND** (a two-key lock) | every move pushes an occurrence into a cell that repels it, so both cells must pay. Truth table over 600 ticks: (0,0) 0 moves, (1,0) 0, (0,1) 0, (1,1) 100 | `a_two_key_lock_is_a_thermal_and_gate` |
| Hot gas | a lone atom holding 1 quantum flies one cell per axis every 3 ticks, (2,2,2) → (4,4,4) in 6 ticks, reflects off walls and keeps its heat. A cold atom never moves. | `a_hot_atom_flies_and_a_cold_one_sits` |
| Determinism | two runs of a burning rod: equal digests after 500 ticks | `two_peers_compute_the_same_bytes` |

## 5. What it cannot do, or does badly

- **Inhibition (NOT) fails.** Heat in a second cell does make a better move affordable, which
  pre-empts the first cell's move. But the pre-empted state is still reached a few moves later:
  in the (1,1) row the "inhibited" state appeared anyway, and so it did with only the second
  cell hot. Measured in
  `inhibition_by_preemption_fails_the_state_is_reached_anyway`. Without inhibition and without
  reusable signals (fuel is consumed, and the lock flickers instead of latching), there is no
  computation beyond monotone, single-use AND/OR.
- **Hot matter mints configurations without end.** A burning 16-column rod (128 cells) goes from
  3 configurations to 26 at t=250, 1002 at t=750, 2043 at t=1250 and 2584 at t=2000
  (`hot_matter_keeps_minting_configurations`). In this matter model, thermal motion is
  compositional churn. This is the main cost problem (section 6).
- **No momentum, no sound.** Heat is a scalar with no direction. Hot gas moves at the causal
  speed in a direction fixed by the parity of its position, not by any velocity.
- **Fire in bulk fizzles.** On the 64³ terrain slab, 16 sparks at 3× ignition burned 1327, 526
  and then 29 moves per 100 ticks, and then stopped. Heat spreads into three dimensions faster
  than the fuel releases it. Only thin fuses (the rod) or a preheated charge sustain a front.
  This is Semenov's criterion working, but a game would want fuels that burn in bulk.
- **No Boltzmann statistics.** The greedy rule always takes the best affordable move. A hot
  contact sits in its best reachable state or flickers (period 12, `a_two_key_lock…` and the
  9000-quanta cycle). It does not sample states by weight. "Temperature" is a per-cell budget,
  not an emergent average.
- **Conduction has a dead band.** Truncation stops the flux below `2^13 / bond` quanta of
  difference, so small gradients never relax. With the bonds used here (58–395), differences
  below about 21–141 quanta between bonded cells are permanent. This follows from the formula and
  is not measured separately.
- **Front speed is irregular.** 0.0463 and then 0.0745 columns/tick in the two halves. The front
  carries local chemistry and does not settle to one exact speed.

## 6. Cost

Bench: 64³ = 262,144 cells, 50% filled (sink rock in y 0–23, oxidizer in y 24–28, fuel seam in
y 29–31). Release build, `nice 10`. The timings are from one run (they vary by a few per cent),
and the move counts and palette sizes are deterministic.

| Phase | ns per cell-tick | cells/s | active cells per tick | ns per active cell | palette |
|---|---|---|---|---|---|
| at rest (1000 ticks) | ≈ 0 (an empty warm list) | 1.3e14 | 0 | n/a | 4 |
| 16 sparks, 300 ticks | 0.065–0.085 | 1.2–1.5e10 | 735–822 | 21–30 | 4 → 11 |
| whole seam at ignition, rounds of 20 ticks | 6.6 → 14.8 → 37.5 → 153 → 554 → 2049 | 1.5e8 → 4.9e5 | 43k–60k | 40 → 73 → 179 → 716 → 2528 → 9021 | 11 → 401 → 701 → 1229 → 1939 → 3038 |

- **The law itself costs 21–56 ns per active cell** (while the palette stays below about 300): six face fluxes, one memo lookup, and a short
  scan of the pruned move list. A cold world costs nothing, because warm cells are the exact
  work set (a cell with no heat can neither conduct nor give).
- **Palette growth then dominates.** law-core's `Matter::intern` rebuilds the dense P×P bond table
  for every new configuration, an O(P²) cost (and 9.7 s of the bench's 14.7 s were system time
  from those reallocations). This package does not work around it: the cells must live in
  `Matter`. law-core would need an incremental, sparse bond cache. The deeper problem is that the
  law makes P grow at all.
- **Memory:** 4 bytes of heat per cell, plus 9 bytes per cell of per-tick scratch. A host can keep
  scratch per active chunk only.
- **At world scale:**
  - Activity tracking is built in (the warm list), and quiet-but-warm cells still cost a
    conduction check.
  - Chunks must share the global tick phase.
  - The server is the authority: integer only, digest-checked.
  - Saves store heat sparsely, as (cell, q) pairs for warm cells.
  - Network deltas are material changes plus a coarse heat overlay, because heat changes every
    tick in warm regions.
  - Configurations need garbage collection or refcounting, or some bound on compositional
    diversity.

## 7. Gameplay

- **The verb:** `poke(i, +s)` is a torch, which puts `s` quanta into a matter cell. `poke(i, −s)`
  is frost, which takes heat out. Both are booked as `work`, so the ledger stays closed. Void
  cannot be heated.
- **What players would discover:**
  - Every pair that selective transfer would have reacted is now a fuel that waits for a spark.
  - Fuses: thin seams burn, and thick ones fizzle.
  - Firebreaks: blocks that repel everything.
  - Preheating turns a slow burn into an explosion.
  - Smelting: heat an ore against a collector past a sharp threshold, then quench it to keep the
    product.
  - Two-key locks that open only when both sides are hot.
  - Ticking clocks: small closed assemblies that oscillate forever, such as the period-90 X F X.
- **What would confuse or frustrate:**
  - Heat is invisible without an overlay.
  - Hot regions churn into a speckle of new materials that keeps changing colour.
  - Fire seems to die for no visible reason in thick terrain.
  - Clocks and locks flicker rather than click.

## 8. Verdict

| Criterion | Score | Why |
|---|---|---|
| Emergence | 4 | Ignition points, fronts, explosions, firebreaks, smelting, relaxation clocks and hot gas all come from one move rule plus conduction. Waves and momentum do not. |
| Generality | 4 | Every constant comes from fit, and any reactive pair is a fuel. Self-conduction needs mixtures, because pure blocks bond 0 to themselves. |
| Expressive power | 2 | AND works and clocks exist. NOT fails (measured), and signals are single-use. |
| Elegance | 4 | One affordability rule makes activation and local energy conservation fall out. It is marred by the dimer schedule and the greedy ranking. |
| Efficiency | 2 | Zero cost at rest and 21–56 ns per active cell, but heat-driven palette growth took a burning seam from 6.6 to 2049 ns per cell-tick within 200 ticks. |
| Enjoyment | 3 | Fire, frost and smelting are intuitive. The churn and the fizzles are not. |
| Extensibility | 4 | The ledger can take more forms of energy, such as vibration (`zeldovich`) or a heat-gated threshold for `belousov`. |

**The strongest objection:** in this matter model, thermal motion *is* compositional entropy.
Any region above its barriers keeps visiting new configurations: 2584 from 3 in 2000 ticks on
128 cells, and 3038 within 200 ticks on the bench seam. A hot world's palette and cost therefore
grow without bound. The demon also makes this worse, not better: it is a bookkeeper, not a heat
bath. Its greedy choice yields no Boltzmann weights, so the thermodynamics is enforced by the
ledger rather than emerging from many degrees of freedom. Lem's Demon of the Second Kind, asked
to sort hot matter, buries you in paper.
