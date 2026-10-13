# Law: Toda (`pwc.law-toda`)

A candidate fundamental law for Project Watt Cubed: **the reversible wave field**. Every block is
a mass on springs. It remembers a displacement and a momentum, and one tick is a palindrome of
exact integer shears. Nothing ever halts by itself, nothing is ever lost, and the whole universe
runs backwards bit for bit. A library: it registers nothing. It depends only on `pwc.law-core`.

Every number below comes from a passing experiment in `src/tests.rs` (named in brackets) or from
the bench, run in the physics lab:

```
cd physics-lab
cargo test --release -p pwc_law_toda -- --nocapture
cargo run  --release -p pwc_law_toda --example bench
```

## 1. Inspiration

- **Fermi, Pasta, Ulam and Tsingou (Los Alamos, 1955).** A chain of masses with a slightly
  nonlinear spring does not thermalise; its energy returns to the first mode. Taken: the
  experiment itself, reproduced on real matter [`fput_recurrence_then_thermalisation`].
- **Morikazu Toda (1967)** and **Zakharov and Faddeev (1971)**, who showed the KdV equation is a
  completely integrable Hamiltonian system, and **Zakharov and Shabat (1972)**: nonlinear lattices
  carry solitons that behave like particles and pass through each other. Taken: solitons as the
  law's moving objects, and the test that they keep their shape and collide elastically.
- **Izrailev and Chirikov (Novosibirsk, 1966)**, *Statistical properties of a nonlinear string*,
  and **Chirikov (1979)**, the standard map `p += K sin θ, θ += p`: the FPUT paradox resolved
  as a threshold between regular motion and chaos. Taken: the law's very form (a kick then a
  drift on a torus, one coupling per bonded face), and the two regimes, recurrence at strain 0.1
  and spreading at 0.3.
- **Kolmogorov (1954), Arnold (1963), Moser (1962).** KAM: quasi-periodic motion survives weak
  nonlinearity. That is why the weak regime recurs.
- **Frenkel and Kontorova (1938)**, and **Frenkel's (1926)** sinusoidal estimate of a crystal's
  shear strength: a bond is periodic, and slipping one period leaves the same crystal. Taken: the
  spring is a smooth periodic function of the displacement difference, so overstrained bonds slip
  and heal [`a_hard_hit_slips_a_bond_and_it_heals`].
- **Arnold and Avez (1968)**, the cat map: an integer area-preserving map of a torus is a
  bijection, so every orbit of a finite system is a cycle. Taken: wrapping arithmetic as the
  geometry of the state, and the exact recurrence of a dimer [`a_dimer_is_an_exact_clock`].
- **Fiction.** The brothers Strugatsky, *Monday Begins on Saturday* (1965): Janus Poluektovich,
  the institute director whose second self lives backwards in time ("contramotion"). Taken: rewind
  and the time mirror as gameplay verbs that the law gives for free. Stanisław Lem, *Solaris*
  (1961): a whole planet that is one vibrating, remembering medium. Taken: the picture of a world
  in which every compatible block rings with everything that touched it.

## 2. What the universe remembers

- Per cell: `u: i32`, a displacement **phase** (an integer modulo 2^32; only differences of `u`
  ever enter the law), and `p: i32`, a momentum. 8 bytes. Void cells stay at `u = p = 0`.
- Per face: nothing. Springs are read from the matter (the bond), which this law never changes.
- Caches (a function of the state and the matter, never hashed): a 1-byte bond mask per cell,
  the cached half impulse at the current `u` (4 bytes), an era mark for the activity lists
  (4 bytes) and the activity worklists (up to 12 bytes per cell at full activity).

## 3. The law

One tick, for every matter cell, is three shears (leapfrog, Störmer–Verlet):

```
p_i += Σ_j F(k_ij, u_j − u_i)        half kick (reads only u)
u_i += G(w_i, p_i)  (mod 2^32)       drift     (reads only p)
p_i += Σ_j F(k_ij, u_j − u_i)        half kick at the new u
```

- `F(k, d) = sign(d) · round(k · S(|d|) / 2^(s+1))`, computed **per bond** and applied with
  opposite signs to both ends. So Newton's third law holds exactly and Σp is conserved exactly.
- `S(a) = 2^31 · h(a / 2^31)` with `h(t) = t + 6t³ − 13t⁴ + 6t⁵` on half a period, extended oddly
  and periodically (period 2^32). `h(0) = h(1) = 0` and `h''(0) = h''(1) = 0`, so the spring is
  C² on the whole torus. Its slope is 1 at rest and rises to 1.46 at `t = 0.3` (FPUT-β hardening,
  which makes solitons possible). It peaks at `t ≈ 0.61` (the bond's strength), then falls to zero
  at half a period, where the bond breaks and pulls towards the next well. The linear term is
  exact in every bit, and the correction uses four integer multiplications at 28 bits.
- `G(w, p) = round(p · w / 2^16)`, odd in `p`.
- Rounding is always half away from zero, so `F` and `G` are odd.

**Why it is exactly reversible.** Each shear adds to one half of the state a function of the
other half, so it is a bijection of `(Z/2^32)^2N`, for every state including absurd ones. The
same holds for a poke (`p += s`). The tick is a palindrome of shears, `F` and `G` are odd, and so
`flip ∘ step ∘ flip = step⁻¹` exactly. `unstep` is implemented in exactly that way. Overflow is
not guarded against, because it does not exist: wrapping is the torus. `u` is a phase and only
wrapped differences enter `F`, whose period is the wrap. A block that drifts forever wraps
harmlessly. The force sums never come near overflow, because a spring's strength is bounded:
a half kick is at most `6 · 1151 · 0.68 · 2^31 / 2^19`, about 1.9e7. `p` is bounded by the energy
in any physical state. A poke near 2^31 simply wraps, and that is still a bijection (tested).

**Every constant from fit:**

| Constant | Derivation |
|---|---|
| stiffness `k_ij` | `max(bond(a, b), 0)`: void, neutral and repulsive faces carry no spring |
| inverse mass `w` | `round(2^21 / len)`: mass is the occurrence count (32 occurrences give `w = 2^16`) |
| time shift `s` | the least `s` with `18 · k_max · w_max < 4 · 2^(s+16)`. Here `k_max = 1151` is the largest bond the fit table can make (read from the table), `w_max` is that of one occurrence, the 18 is `2 · 6 faces · 3/2` (the spring's steepest slope), and 4 is leapfrog's `ω < 2` (Gershgorin bound). That gives `s = 18` [`materials_derive_springs_and_masses_from_fit`] |
| sound speed | `c = sqrt(k · w / 2^(s+16))` cells per tick, with no free parameter |

No material is named and no exception exists. The update order is irrelevant within each shear,
so the law is synchronous without a double buffer.

**Activity.** A cell with `p = 0` and zero impulse is exactly at rest and is skipped. A hot list
holds a superset of the cells with momentum or impulse. A tick touches the hot cells and the
bonded neighbours of the cells that moved. When a quarter of the matter is hot, the law switches
to a dense sweep in index order that evaluates each spring once. It returns to the list when
activity falls below an eighth. Neither path changes a bit: both are checked against a cache-free
reference sweep, tick by tick [`hot_list_matches_the_full_sweep`, `reversal_is_exact_bit_for_bit`].

**Deviations from the brief's spec, and why.**

1. *A periodic spring instead of an FPUT-β cubic with saturation.* The first build used a cubic
   spring that yielded to a constant tension. Its kink at the yield point and its non-periodic
   wrap made energy wander by percents at moderate amplitude. That was measured during
   development and not kept as a test. The periodic C² spring removes every special state: no
   saturation, no overflow argument, and a shadow energy that holds to 0.015%.
2. *Kick-drift-kick instead of kick-drift.* Kick-drift is already exactly invertible. The
   palindrome is also time-symmetric, which gives a Loschmidt echo with nothing but a momentum
   flip and a second-order energy error. The plain energy band in the conservation experiment
   fell from 10.6% (kick-drift, measured during development) to 1.23%. The cost per tick is unchanged: the first half kick reuses the impulse
   cached by the previous tick's second half kick.
3. *No exponential (Toda) potential.* An exponential is not integer-friendly, and its hard wall
   breaks the universal stability bound. The candidate keeps the name, and its solitons are those
   of a hardening lattice (Friesecke–Wattis).

## 4. What emerges

| Phenomenon | Measurement | Experiment |
|---|---|---|
| Exact reversibility | 40 pokes from 300 to `i32::MAX − 7` (wrapping momentum) in a 14×12×10 slab with void, a pure layer and repulsive faces. Runs 5000 ticks forward and 5000 back, and every intermediate digest matches the forward run. Tracked law = reference sweep for 600 ticks. Arbitrary 32-bit `u, p` everywhere: 500 ticks there and back reach the identical state | `reversal_is_exact_bit_for_bit` |
| Time symmetry | `flip; 2000 × step; flip` returns the exact starting digest | `time_mirror_retraces_the_past` |
| Conservation | Σp equals the summed pokes at every sample for 60 000 ticks. In a 16³ slab the shadow energy stays in a 0.0148% band and the plain energy in 1.23%. Secular drift −0.0004% | `momentum_is_exact_and_energy_stays_bounded` |
| Sound speed = sqrt(k/m) | Three mixtures (len 2/8/16, k 515/373/357). Predicted 0.17729/0.07544/0.05219 cells per tick, measured 0.17716/0.07539/0.05215 (−0.08%). Fastest/slowest 3.40 | `sound_speed_follows_stiffness_over_mass` |
| Impedance mismatch | Light-to-heavy line (Z ratio 2.33, interface bond 181): 0.1688 of the energy reflected, against 0.1598 from `((Z₁−Z₂)/(Z₁+Z₂))²`. An incompatible face (bond −113) reflects everything (1.0001), and the far side never moves by one unit | `impedance_mismatch_reflects_as_predicted` |
| Resonator | A struck 3³ cube in void rings for 200 000 ticks. Shadow band 0.14%, corner swing 3.02e8 early and 3.10e8 late, kinetic share 0.513 (virial ½) | `resonator_rings_forever` |
| An exact clock | A dimer oscillates with a 24.82-tick period (linear leapfrog theory 24.99; the hardening shortens it). The **whole state recurs bit for bit after 2 042 402 ticks**: a bounded orbit of a bijection is a cycle | `a_dimer_is_an_exact_clock` |
| Soliton | An end kick on a 1600-cell line. The leading pulse keeps its height (0.1245 → 0.1221 of half a period, ×0.98) and width (5 → 5 cells) over 6000 ticks, and is supersonic (+1.8%). The same kick ×1/100 (linear) disperses: height ×0.63, width 8 → 13, speed −0.2% | `soliton_keeps_its_shape_where_linear_waves_spread` |
| Elastic collisions | Head-on solitons cross (strain 0.234 at the crossing) and leave with their height (0.1183 alone, 0.1241 after) and a +0.23-cell phase shift | `solitons_collide_elastically` |
| FPUT recurrence | 32 blocks, all energy in mode 1 at strain 0.1. Mode 1 falls to 0.19 by tick 15 000 and is back above 0.95 at tick 33 500 (0.99 near tick 180 000). Late effective modes: 2.2 of 31 | `fput_recurrence_then_thermalisation` |
| Thermalisation | Same line at strain 0.3: mode 1 drops to 0.015 and never returns. Late effective modes: 10.9 of 31 | `fput_recurrence_then_thermalisation` |
| Heat as statistics | One blow in a 16³ slab: the participation ratio climbs from 0.0005 to 0.23–0.31 (an incoherent buzz). Rewinding 20 000 ticks reassembles the blow exactly (digest equal, ratio 0.0005) | `heat_is_only_statistical` |
| Fracture that heals | A 1e7 kick on a line end leaves the bond intact. A 3e7 kick slips the end bond 60 times by tick 709, then the bond re-binds in the next well and the spin leaves as sound: the end block keeps 1% of the energy. Shadow drift −0.7% through the event | `a_hard_hit_slips_a_bond_and_it_heals` |
| Rewind (gameplay) | Poke, 1500 ticks, 1500 `unstep`s through the lab's `Candidate`, unpoke: the pristine digest, with momentum 0 | `rewind_undoes_play` |

## 5. What it cannot do, or does badly

- **No gain, no logic.** The best attempt is a Y junction feeding a weakly bonded link block
  (bond 52), which acts as a fuse [`junction_logic_attempt`]. Gentle pulses superpose: two inputs
  give 4.34× the output of one (1.23e14 against 2.83e13, coherent addition). Hard pulses
  together slip the fuse: two inputs give 0.71× the output of one (1.93e14 against 2.71e14).
  That is an XOR-like contrast of 1.4:1, but the output is at most 21% of the input energy. A
  conservative passive medium has no stored free energy to release, so nothing can be cascaded.
  Soliton collisions are elastic (a 0.23-cell shift), so solitons do not compute either.
- **Matter never changes.** The law moves no block and makes no chemistry. `u` is an invisible
  internal phase, shown only as a heat map.
- **Pure matter is deaf.** Identical elements fit 0, so a pure block has no spring to its own
  kind. A poked block inside a pure layer drifts forever alone (Newton's first law). After 1000
  ticks it is the only disturbed cell of 4096
  [`rest_costs_nothing_and_unbonded_matter_is_deaf`].
- **Nothing dissipates.** A poked region never comes back to rest, which is the price of
  exactness. Activity tracking saves work only until the first touch. After one poke the 16³
  slab needs the dense sweep within 100 ticks. In the bench, 16 pokes push the 64³ box into the
  dense sweep within 200 ticks.
- **Sound is slow.** The universal stability bound is set by the stiffest, lightest matter the fit
  table allows (top speed 0.375 cells per tick). Typical mixtures run at 0.05–0.18 cells per tick,
  which is 1–3.5 cells per second at 20 Hz.
- **A scalar field.** `u` has one component. There is no shear versus pressure wave and no vector
  motion. The cubic lattice is anisotropic at short wavelengths (not measured).
- **Not exactly energy-conserving.** Momentum is exact; energy is conserved only in the shadow
  sense (0.015%). An integer bijection cannot also keep a real-valued Hamiltonian exactly.

## 6. Cost

Bench: i5-12400F, one thread, `nice -n 10`, one run (repeat runs vary by about ±5%). The box is 64³ (262 144 cells) with 125 369 matter
cells (47.8%) as terrain-like hills: three mixtures in strata, pure pockets and caves.

| Phase | ns per cell-tick | cells/s | active cells | per tick |
|---|---|---|---|---|
| at rest | 0.000 | (no work) | 0 | 0 µs |
| 16 pokes, first 200 ticks | 5.04 | 1.99e8 | 153 872 | 1.32 ms |
| 16 pokes, next 2000 ticks | 7.31 | 1.37e8 | 262 144 (dense) | 1.91 ms |
| the same 2200 ticks backward | 7.40 | 1.35e8 | 255 809 | 1.94 ms |
| every block kicked | 7.28 | 1.37e8 | 262 007 | 1.91 ms |
| busy, backward | 7.47 | 1.34e8 | 262 144 | 1.96 ms |

Both rewinds return the exact digest. Backward costs the same as forward, because `unstep` is
the mirrored `step`. A dense tick costs about 15 ns per matter cell. State is 8 bytes per cell,
plus 9 bytes of caches and worklists that scale with activity. A busy cell carries three springs
(each evaluated once) and two drift roundings.

**At world scale:**

- Each shear is a pure map, so it is embarrassingly parallel and deterministic across threads.
- Chunks need one face layer of `u` from neighbours per tick.
- A save is `u, p` (8 bytes per cell). Untouched cells are zeros and compress to nothing.
- The server is authoritative, or peers run lockstep on pokes alone, since the law is integer and
  deterministic. A joining peer needs the full `u, p` of the busy region.
- The killer is that activity never decays. A host must either let busy regions accumulate or
  add an absorbing boundary (for example damping in unloaded chunks), and that breaks exact
  reversibility at the boundary.
- Real-time sound needs about 10 substeps per 20 Hz tick. By the bench rate, a busy 128³ region
  would cost about 150 ms per game tick on one core (an estimate, not measured).

## 7. Gameplay

- **The verb `poke`** strikes a block: it kicks its momentum. Players hear and see (heat map)
  the blow travel through compatible matter, reflect at mismatches and stop dead at incompatible
  joints, which are perfect acoustic insulators.
- **Discover and build:** bells and gongs (isolated bodies ring forever and are exact clocks),
  speaking tubes along lines of springy mixture, impedance-matched waveguides, and a fuse that
  slips when struck too hard and heals itself.
- **Rewind** (`unstep`) and the **time mirror** (`flip`) fall out of the law. Struck chaos can be
  unscrambled exactly, which is a strong puzzle mechanic (an egg that unbreaks).
- **Confusing or frustrating:** nothing visible moves, since blocks never change position. Pure
  blocks are deaf. Every poke leaves a permanent buzz (heat), and in multiplayer rewinding
  someone else's work needs a server policy. No machine can be built, because there is no gain.

## 8. Verdict

| Criterion | Score | Why |
|---|---|---|
| Emergence | 3 | Sound, resonance, solitons, FPUT recurrence, thermalisation, statistical heat and self-healing fracture all come from one rule, but no chemistry and no change of matter |
| Generality | 4 | Every constant is derived from bond and occurrence count, with no exceptions; pure matter being deaf is a real hole |
| Expressive power | 2 | No gain, no latch; the best gate is an XOR-like fuse with 1.4:1 contrast and at most 21% output |
| Elegance | 5 | A palindrome of three integer shears on a torus with one C² polynomial spring. Reversible for every state, time-symmetric, Σp exact, no overflow case |
| Efficiency | 3 | Zero cost at rest and 1.4e8 cells/s busy on one thread, with 8 bytes per cell. But nothing ever rests again after a touch, and sound needs substeps |
| Enjoyment | 3 | Rewind, ringing and travelling solitons are delightful, but nothing visible moves |
| Extensibility | 4 | `u` and `p` are a clean substrate for zeldovich (reactions paying from vibration), for a vector field or for displacement-driven motion |

**The strongest objection:** this is a beautiful wave equation painted onto static matter. It
conserves everything, so it can never amplify. With nothing to amplify, it can never build a
machine. With nothing that dissipates, every touch leaves the world buzzing forever, which spends
the rest that a voxel world's efficiency depends on. Exactness buys rewind at the price of rest.

## Licence

`Apache-2.0 OR MIT`, at your option.
