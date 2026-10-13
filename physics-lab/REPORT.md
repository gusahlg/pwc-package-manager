# Six candidate universes: what the experiments say

Six candidate laws were built (2026-10-13) on the same ground (`pwc.law-core`: a box of the
game's real configurations, with every constant derived from fit). Each one was tested only
against the phenomena it claims, and every number below comes from a passing test or a bench in
that law's package. Each package README has the full account: inspiration, rules, measurements,
failures, cost and verdict. [BRIEF.md](BRIEF.md) holds the questions and
[SOURCES.md](SOURCES.md) the sources. Run everything from this directory with
`cargo test --release --workspace`.

The **Score** column is each law's own scorecard total out of 35, the sum of the brief's seven
criteria scored 1–5. Every builder scored their own law, so read the totals as a rough guide.

| Law | Remembers per cell | Rule | Score | It earned | It cannot |
|---|---|---|---|---|---|
| `demon` | heat `u32` | selective transfer paid from each cell's own heat, plus exact conduction | 23 | exact energy ledger; sharp ignition threshold (377 vs 376); flame front 0.057 cells/tick; smelting only above 3000 quanta; firebreaks; a self-cycling 3-cell box (period 90); AND | NOT; sustained 3-D fire; sound; bounded cost (palette 3 → 2584 in 2000 ticks) |
| `belousov` | phase `u8` | fire when glow + Σ bond·excited > the emission onset; recover over P = 3 + cohesion/256 | 27 | signals at 1 cell/tick with no loss; diode; clocks; AND/OR/NOT and a half adder; ring memory for 10k ticks; spirals; fan-out; 0 cost at rest | conserve anything; change matter; start spirals by itself; vary signal speed; 84% of matter is inert |
| `toda` | `u`, `p` (8 B) | kick–drift–kick integer shears with a periodic spring | 24 | **bit-exact rewind** over 5000 ticks; momentum exact; sound speed within 0.08%; solitons; FPUT recurrence and thermalisation; fracture | gain; changing matter; returning to rest |
| `margolus` | particle bit | one derived 2×2×2 block bijection, fully symmetric | 23 | exact rewind; AND gate and crossover in open 3-D; 0.02–0.4 ns/cell-tick | **diffusion**: proved impossible for any momentum-conserving one-speed rule here; matter is inert |
| `zeldovich` | 6 directed quanta counts + rotor (25 B) | one face rule (energy pays for transfer) + one cell rule (deal quanta over conducting faces) | 26 | exact ledger over 10k ticks; equipartition to 3 digits; detonation 1.000 cell/tick with **logic-level restoration** (12/24/36 in → 28 out); ignition delay (Semenov); amplifier 12 → 20; battery; AND/OR | NOT; reusable fuel; reversibility; phase; bounded fire (beds burn to 491k quanta) |
| `polyakov` | `D` faces, `B` edges, `ρ` (14 B) | integer Yee scheme on Z₆₅₅₃₆, plus Ohm | 22 | Gauss and div B exact; Fresnel to 3 digits; capacitor, LC ring, induction, shielding; stable monopoles | gain (gates pass 17–27%); rest; changing matter; exact 3-D integer Yee (rounding heats vacuum) |

## Five lessons that hold across all six

**1. No law has gain, changeable matter and rest together.** These three properties separate
the candidates:

- **gain**, a small cause releasing a large effect, without which logic cannot cascade;
- **changeable matter**, without which there is no chemistry, crafting or wear;
- **rest**, a disturbed region returning to zero cost, without which a massive world cannot run.

| Law | Gain | Changes matter | Returns to rest |
|---|---|---|---|
| belousov | yes (drive) | no | yes |
| zeldovich | one-shot (fuel) | yes | partly (heat spreads) |
| demon | weak | yes | partly |
| toda, margolus, polyakov | no | no | **no** |

The requirements for a winning law are in this table.

**2. Reversibility forbids rest, and that is a theorem rather than a bug.** Every exactly
reversible law (`toda`, `margolus`, the field part of `polyakov`) left the poked region busy
forever. By Liouville, a bijection cannot shrink phase space, so a disturbed region can never
settle to rest unless the disturbance leaves. A reversible world is therefore busy wherever
anyone has ever touched it, and its cost grows with history. The only clean escape is an
**open boundary**: space as a sink that radiation leaves through and never returns from. Rewind
is the prettiest feature in the lab. It costs exactly the sparsity a world-scale host depends on.

**3. Gain comes only from stored free energy, and the excitable medium is simply renewable
fuel.** `belousov` gets restoration and cascading logic because every cell recovers for free,
which is an unpaid drive. `zeldovich` gets the same logic-level restoration (any trigger
detonates to exactly 28) from metastable binding, but its fuel is spent after one use. These are
one phenomenon. An excitable medium *is* a detonable medium that recharges, which is how the
Belousov–Zhabotinsky reaction behaves. So `belousov` need not be a separate law. It should
*emerge* from `zeldovich` once something recharges the fuel. The game already has the
candidates for that: glowing matter, the sun and the sky.

**4. The matter model's unsaturated binding makes every hot world explode.** `demon` and
`zeldovich` both found it independently. Binding is the sum of fit over every internal pair, so
it grows like n² and never saturates. Heat lets compositions wander, the clumps that bind most
always win, and the number of distinct configurations grows without bound (`demon` reached
2584; `zeldovich` 12,817, which spent 94% of its time computing bonds until law-core switched to
lazy bonds). That flaw belongs to the representation, not to these laws. The real-chemistry
answer is **valence**: each occurrence binds at most k partners (Mendeleev, Lewis). Saturation
bounds the palette, gives molecules a natural size and makes fuel beds burn to a critical size
instead of without limit. This is the one change the experiments ask of the core matter model.

**5. Directed quanta are the right carrier, and conservation alone cannot make heat spread.**
`margolus` proved that momentum-conserving one-speed particles on a cubic lattice never diffuse.
`zeldovich` found that a face rule alone can never turn a quantum's axis. Both are forced to the
same answer: a **deterministic scatterer** in matter. That is `zeldovich`'s doubly stochastic
rotor dealing, Sinai's dispersing billiard written in integers. It gives correct equipartition
with no randomness: God does not play dice, and the dealing is deterministic chaos. Clear matter,
void and space should keep ballistic quanta, which is light.

## Recommendation

There is no winner as built. There is one clear direction: **zeldovich as the core**, the only
candidate that has gain, changes matter and keeps an exact single-energy ledger with sparse cost.
Make three changes, each named by an experiment above:

1. **Valence-saturated binding** in the matter model (lesson 4). This bounds the palette and the
   fire, and it is the only core change.
2. **A renewable drive and a sink.** Glowing matter emits quanta, and quanta that reach space
   leave the world (lessons 2 and 3). The test is whether `belousov`'s clocks, wires, diodes and
   spirals then *emerge* in recharging fuel without a phase rule. If they do, the universe
   needs one quantity and two local rules, with nothing borrowed from `belousov`. That
   experiment is decisive.
3. **NOT through refractoriness.** In `belousov`, NOT and the half adder work because a just-fired
   cell cannot fire again. A spent-and-recharging fuel cell has the same property for free, so
   inhibition need not be a rule. Test it once change 2 exists.

`toda`'s rewind and `polyakov`'s fields stay as research. They are the two most elegant laws
here, and both cost the world its rest (lesson 2). `margolus`'s ballistic engine (0.02 ns/cell-tick)
is the right way to carry light through void inside `zeldovich`.

## What is not settled

None of this has met a player. `pwc.physics-lab` (`/law <id>`) runs every candidate on the real
terrain so that the next judgement can be made in game, not on paper. The scores are the
builders' own and should be treated as such. Nothing is wired into the server, saves or the
network. The core extraction (`notes/2026-10-13-simulation/extraction-review.md` in the game) is
the seam a chosen law would use.
