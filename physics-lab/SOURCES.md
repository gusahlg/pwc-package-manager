# Sources: Soviet mathematics, physics and science fiction for the PWC fundamental law

<!-- Annotated bibliography, 2026-10-13. Purpose: inspiration and grounding for the candidate
laws in BRIEF.md. Citations were checked against publisher pages, MathNet.ru, ADS, Springer,
Project Euclid, Scholarpedia and similar sources. Where a detail could not be confirmed it is
marked "not re-checked". Russian titles are transliterated; "ДАН" = Doklady Akademii Nauk SSSR,
"УМН" = Uspekhi Matematicheskikh Nauk, "ЖЭТФ" = Zhurnal Eksperimental'noi i Teoreticheskoi
Fiziki. -->

The owner wants one simple deterministic rule from which the most complex things emerge. The
Soviet school is unusually rich for this: it gave the world excitable media, autowaves, KAM,
resonance overlap, inverse scattering, the tetrahedron equation, dispersing billiards, stable
probabilistic cellular automata and algorithmic complexity. Most of it was built on paper or on
small machines, so it is naturally about integer, local, low-cost rules.

Candidate numbers used below:

1. Microcanonical energy-ledger chemistry (heat quanta, barriers, flames).
2. Excitable medium (phase cycles, thresholds, spirals, pacemakers).
3. Reversible integer wave/lattice field with nonlinear springs (solitons, FPUT, KAM).
4. Reversible partitioned particle cellular automaton (billiard-ball computer, lattice gas).
5. Synthesis: chemistry plus vibration with one conserved energy, gain from metastable fuel.
6. Lattice gauge field on faces/edges (charge, flux, exact Gauss law).

---

## Part A. Excitable media, oscillating chemistry and autowaves

### Belousov (1959): the oscillating reaction
B. P. Belousov, "Periodicheski deistvuyushchaya reaktsiya i ee mekhanizm" [A periodically acting
reaction and its mechanism], *Sbornik referatov po radiatsionnoi meditsine za 1958 g.*
[Collection of abstracts on radiation medicine for 1958], Medgiz, Moscow, 1959, pp. 145–147.
English translation in R. J. Field and M. Burger (eds.), *Oscillations and Traveling Waves in
Chemical Systems*, Wiley, New York, 1985, pp. 605–613.

A citric-acid/bromate/cerium mixture switches colour back and forth for an hour. Journals
rejected it because "a closed system cannot oscillate"; this obscure abstract was the only
version Belousov published. The oscillation is not a violation of thermodynamics: it rides on
a monotone fall of free energy, the way a waterwheel turns on a falling river.
→ use in PWC: (2) and (5). The proof that a clock can run on a draining fuel reservoir is the
whole justification for "gain from metastable fuel"; the law must let free energy fall
monotonically while some other variable cycles.

### Zhabotinsky (1964): kinetics of the Belousov reaction
A. M. Zhabotinsky, "Periodicheskii protsess okisleniya malonovoi kisloty v rastvore
(issledovanie kinetiki reaktsii Belousova)" [Periodic oxidation of malonic acid in solution: a
study of the kinetics of Belousov's reaction], *Biofizika* 9 (1964) 306–311.

Zhabotinsky replaced citric with malonic acid, measured the cycle and identified the
autocatalytic (fast) and inhibitor (slow) steps. This split into one fast activator and one
slow recovery variable is the template behind every later excitable-media model.
→ use in PWC: (2). Use exactly two state variables per cell, a fast "excitation" and a slow
"recovery", and derive both rates from matter readings.

### Zaikin & Zhabotinsky (1970): spirals in a dish
A. N. Zaikin and A. M. Zhabotinsky, "Concentration wave propagation in two-dimensional
liquid-phase self-oscillating system", *Nature* 225 (1970) 535–537. doi:10.1038/225535b0.

An unstirred thin layer of BZ solution shows target rings from pacemaker spots and, when a
wave is broken, rotating spirals. Waves annihilate on collision instead of passing through
each other. The spiral is a self-organized pacemaker: no special site is needed.
→ use in PWC: (2). Spirals are the test of a good excitable law; the player's "break the
wave" verb (a `poke` that erases a stretch of front) should produce a spiral pair.

### Wiener & Rosenblueth (1946): the cellular excitable model (context)
N. Wiener and A. Rosenblueth, "The mathematical formulation of the problem of conduction of
impulses in a network of connected excitable elements, specifically in cardiac muscle",
*Archivos del Instituto de Cardiología de México* 16 (1946) 205–265.

Not Soviet, but it is the starting point the Moscow school answered. Each element has three
states: rest, excited, refractory. Waves circulate around obstacles ("circus movement"), but
Wiener and Rosenblueth argued that without an obstacle there is no re-entry.
→ use in PWC: (2). The three-state rest/excited/refractory cycle is the cheapest possible law
and the baseline any excitable candidate must beat.

### Gelfand & Tsetlin (1960): continuum models of excitable media
I. M. Gel'fand and M. L. Tsetlin, "O kontinual'nykh modelyakh upravlyayushchikh sistem" [On
continuum models of control systems], *ДАН СССР* 131(6) (1960) 1242–1245. MR0118543.

Gelfand's seminar abstracted the heart into a medium of identical elements with a
threshold, an excitation and a refractory time, without tissue-specific detail. They studied
waves in a line, a ring and the plane. This turned cardiology into a branch of dynamical
systems and trained Krinsky and the Pushchino school.
→ use in PWC: (2). Exactly the brief's rule "no material is named": a medium is defined by a
few numbers (threshold, refractory length, coupling), which can all be derived from fit
readings.

### Krinsky (1966): wave breaks and fibrillation
V. I. Krinsky, "Rasprostranenie vozbuzhdeniya v neodnorodnoi srede (rezhimy, analogichnye
fibrillyatsii serdtsa)" [Spread of excitation in an inhomogeneous medium: regimes similar to
cardiac fibrillation], *Biofizika* 11 (1966) 676–683.

In a medium whose refractory time varies from place to place, a wave breaks on a
long-refractory patch, its free ends curl into spirals and the spirals multiply into
turbulence. Krinsky showed that fibrillation needs no obstacle, only heterogeneity. Later he
edited *Self-Organization: Autowaves and Structures Far from Equilibrium* (Springer, 1984).
→ use in PWC: (2). Heterogeneous matter is free in PWC (every block is different), so the
law will get spiral chaos "for free"; this paper predicts when, which lets us tune it.

### Kolmogorov, Petrovsky & Piskunov (1937): fronts that invade
A. N. Kolmogorov, I. G. Petrovsky and N. S. Piskunov, "Étude de l'équation de la diffusion avec
croissance de la quantité de matière et son application à un problème biologique", *Bulletin de
l'Université d'État à Moscou, Série Internationale, Section A* 1(6) (1937) 1–25.

A quantity that grows locally and diffuses forms a travelling front with a minimum speed
fixed by growth rate and diffusivity (c = 2√(rD) for the logistic case). The speed is set at
the leading edge, where the invader is rare. Published the same year as Fisher's paper.
→ use in PWC: (1) and (2). Gives a closed formula to check a candidate's front speed against;
also a design for spreading phenomena (rot, rust, moss) from one growth-plus-spread rule.

### Zeldovich & Frank-Kamenetskii (1938): the thermal flame
Ya. B. Zel'dovich and D. A. Frank-Kamenetskii, "A theory of thermal propagation of flame",
*Acta Physicochimica URSS* 9 (1938) 341–350.

A premixed flame is a front where heat diffuses forward and ignites fuel whose reaction rate
is a steep Arrhenius function of temperature. In the high-activation limit the reaction lives
in a thin sheet and the speed is fixed by heat diffusion into it (the "ZFK" speed). Unlike
KPP, the speed is set by the hot interior, not the leading edge ("pushed" vs "pulled" fronts).
See also Ya. B. Zeldovich, G. I. Barenblatt, V. B. Librovich and G. M. Makhviladze, *The
Mathematical Theory of Combustion and Explosions* (Nauka, 1980; Plenum, 1985).
→ use in PWC: (1) and (5). An activation barrier in quanta is the integer Arrhenius; a
correct ledger law should reproduce a pushed flame whose speed depends on the barrier height.

### Semenov (1934): branching chain reactions
N. N. Semenov, *Tsepnye reaktsii* [Chain reactions], Goskhimtekhizdat, Leningrad, 1934; English
*Chemical Kinetics and Chain Reactions*, Oxford University Press, 1935. Nobel Prize in
Chemistry 1956.

Explosions happen when each active centre creates more than one new centre on average, so
there is a sharp limit between slow burning and explosion. Thermal explosion and chain
branching are two different routes to runaway.
→ use in PWC: (1) and (5). A "radical" token that splits on reaction gives explosions and
sharp ignition limits without any explosion rule.

### Merzhanov & Borovinskaya (1972): solid flames (SHS)
A. G. Merzhanov and I. P. Borovinskaya, "Samorasprostranyayushchiisya vysokotemperaturnyi
sintez tugoplavkikh neorganicheskikh soedinenii" [Self-propagating high-temperature synthesis of
refractory inorganic compounds], *ДАН СССР* 204(2) (1972) 366–369.

A pressed mixture of powders (say titanium and carbon), lit at one end, burns as a "solid
flame" and leaves a new ceramic behind. Fronts can be steady, pulsating or spinning
(spin combustion), depending on one dimensionless number.
→ use in PWC: (1)/(5). Crafting by ignition: pack two materials, light one end, get a third.
Pulsating and spinning fronts are free emergent tests.

### Andronov & Khaikin (1937): self-oscillation
A. A. Andronov and S. E. Khaikin, *Teoriya kolebanii* [Theory of oscillations], ONTI, Moscow,
1937; second edition with A. A. Vitt, 1959; English *Theory of Oscillators*, Pergamon, 1966.
Vitt co-wrote the first edition but was arrested in 1937 and left off its title page.

Introduced "avtokolebaniya" (self-oscillations): sustained oscillations whose amplitude is
set by the system, not by initial conditions, mathematically a stable Poincaré limit cycle.
A self-oscillator needs a source of energy, a valve and feedback.
→ use in PWC: (2) and (5). "Source, valve, feedback" is a design checklist: the law needs
all three somewhere for clocks to exist.

### Andronov & Pontryagin (1937): rough systems
A. A. Andronov and L. S. Pontryagin, "Systèmes grossiers", *ДАН СССР* 14 (1937) 247–250.

A system is "rough" (structurally stable) if small changes to the equations do not change its
qualitative picture. Rough systems are the only ones a physicist can trust, because real
parameters are never exact.
→ use in PWC: all candidates. A "perfect" law should be rough: its emergent zoo must survive
small changes of the derived constants, otherwise palette tweaks (already known to be
fragile) will break the world.

### Lyapunov (1892): stability and Lyapunov functions
A. M. Lyapunov, *Obshchaya zadacha ob ustoichivosti dvizheniya* [The general problem of the
stability of motion], Kharkov Mathematical Society, 1892. French 1907 (*Annales de la Faculté
des Sciences de Toulouse*); English translation by A. T. Fuller, *International Journal of
Control* 55(3) (1992) 531–773.

Stability can be proved without solving the equations if one finds a function that never
increases along motion (the "second method"). The flip side, which BRIEF.md already uses: a
law with a strict Lyapunov function can only settle, never oscillate.
→ use in PWC: diagnostic for every candidate. Test for hidden Lyapunov functions; any found
means that region will die. Keep one only where death is wanted (e.g. rubble settling).

### Kolmogorov (1936): predator–prey cycles
A. N. Kolmogorov, "Sulla teoria di Volterra della lotta per l'esistenza", *Giornale
dell'Istituto Italiano degli Attuari* 7 (1936) 74–80.

Kolmogorov generalized Lotka–Volterra to arbitrary growth functions and gave conditions for
a stable limit cycle rather than Volterra's fragile neutral cycles. This is the robust,
"rough" version of predator–prey oscillation.
→ use in PWC: (2) or a new ecology layer. Two interacting populations (e.g. two fuel species)
give rough cycles if the law has the Kolmogorov structure.

### Gause (1934): competitive exclusion
G. F. Gause, *The Struggle for Existence*, Williams & Wilkins, Baltimore, 1934.

Experiments with *Paramecium* and yeast: two species living on one limiting resource cannot
coexist; one wins. Coexistence needs different niches. Gause also tested Volterra's equations
in the lab.
→ use in PWC: necroevolution (see Lem). If replicating structures ever emerge, they need
several distinct resources (elements, heat, light) or the world collapses to one winner.

### Sharkovsky (1964): ordering of periods
A. N. Sharkovsky, "Sosushchestvovanie tsiklov nepreryvnogo preobrazovaniya pryamoi v sebya"
[Coexistence of cycles of a continuous map of the line into itself], *Ukrainskii
Matematicheskii Zhurnal* 16(1) (1964) 61–71. English: *International Journal of Bifurcation and
Chaos* 5(5) (1995) 1263–1273.

For a continuous map of the line, existence of a period-3 orbit forces orbits of every period,
following a fixed ordering 3 ▷ 5 ▷ 7 ▷ … ▷ 4 ▷ 2 ▷ 1. Eleven years before Li and Yorke.
→ use in PWC: a cheap test for (2). Find one period-3 cell orbit in a candidate and you know
the law can produce every rhythm.

### Prigogine (dissipative structures)
G. Nicolis and I. Prigogine, *Self-Organization in Nonequilibrium Systems: From Dissipative
Structures to Order through Fluctuations*, Wiley, New York, 1977. Also I. Prigogine and R.
Lefever, "Symmetry breaking instabilities in dissipative systems. II", *Journal of Chemical
Physics* 48 (1968) 1695–1700 (the "Brusselator"). Prigogine was born in Moscow in 1917 and
received the Nobel Prize in Chemistry in 1977.

Far from equilibrium, a flow of energy and matter can sustain ordered states (patterns,
oscillations) that cost entropy production. The Brusselator is a two-species minimal model
showing both Turing patterns and oscillations.
→ use in PWC: (2) and (5). Order should exist only where a flux passes through; when the
source is cut, structures should decay. This is a clear, testable behavioural spec.

### Turing patterns in the Soviet school
A. M. Turing, "The chemical basis of morphogenesis", *Phil. Trans. R. Soc. Lond. B* 237 (1952)
37–72, entered Soviet work through the "autowave" programme: A. M. Zhabotinsky,
*Kontsentratsionnye avtokolebaniya* [Concentration self-oscillations], Nauka, Moscow, 1974; and
V. A. Vasil'ev, Yu. M. Romanovskii, D. S. Chernavskii and V. G. Yakhno, *Avtovolnovye protsessy v
kineticheskikh sistemakh*, Nauka, 1987; English *Autowave Processes in Kinetic Systems*, Reidel,
1987. (Bibliographic detail of the two books is standard but was not re-checked online.)

Russian authors put Turing patterns, BZ waves, fronts and spirals into one taxonomy called
"autowaves": waves whose shape and speed are set by the medium, not by initial conditions, and
which do not reflect or interfere.
→ use in PWC: (2). Adopt the autowave taxonomy (fronts, pulses, spirals, standing patterns)
as the acceptance checklist for any excitable candidate.

---

## Part B. Hamiltonian chaos, solitons and integrability

### Kolmogorov (1954) and Arnold (1963): KAM
A. N. Kolmogorov, "O sokhranenii uslovno-periodicheskikh dvizhenii pri malom izmenenii funktsii
Gamil'tona" [On the conservation of conditionally periodic motions under small perturbation of
the Hamiltonian], *ДАН СССР* 98(4) (1954) 527–530.
V. I. Arnold, "Dokazatel'stvo teoremy A. N. Kolmogorova o sokhranenii uslovno-periodicheskikh
dvizhenii pri malom izmenenii funktsii Gamil'tona", *УМН* 18(5) (1963) 13–40; English *Russian
Mathematical Surveys* 18(5) (1963) 9–36. (J. Moser proved the twist-map case in 1962.)

Most quasi-periodic motions of an integrable system survive a small perturbation, as long as
their frequencies are "sufficiently irrational". So weakly nonlinear systems do not thermalize:
order coexists with chaos.
→ use in PWC: (3). KAM explains why a weakly nonlinear spring lattice can keep a clean wave
for a very long time. Stiffness nonlinearity is the knob between "crystal rings" and "heat".

### Izrailev & Chirikov (1966): the FPU string becomes statistical
F. M. Izrailev and B. V. Chirikov, "Statisticheskie svoistva nelineinoi struny" [Statistical
properties of a nonlinear string], *ДАН СССР* 166(1) (1966) 57–59; English *Soviet Physics
Doklady* 11 (1966) 30–32.

FPU's chain failed to thermalize because its energy was below a threshold. Using resonance
overlap, Izrailev and Chirikov predicted a critical energy above which the chain becomes
chaotic and goes quickly to equipartition. So "heat" in a lattice is a strong-coupling
phenomenon.
→ use in PWC: (3) and (5). Gives the formula for when vibration in a block turns into heat.
Below threshold: sound and solitons; above it: temperature.

### Chirikov (1979): standard map and resonance overlap
B. V. Chirikov, "A universal instability of many-dimensional oscillator systems", *Physics
Reports* 52(5) (1979) 263–379. doi:10.1016/0370-1573(79)90023-1.

The review that made the standard map p' = p + K sin x, x' = x + p' the model of Hamiltonian
chaos. Global chaos starts when neighbouring resonances overlap (K ≈ 1). In more than two
degrees of freedom, motion slowly drifts along resonance webs ("Arnold diffusion").
→ use in PWC: (3). An integer standard map (with a lookup-table sine) is a one-line,
reversible, area-preserving per-cell oscillator; K becomes a derived material constant.

### Arnold's cat map: integer symplectic chaos
V. I. Arnold and A. Avez, *Problèmes ergodiques de la mécanique classique*, Gauthier-Villars,
Paris, 1967; English *Ergodic Problems of Classical Mechanics*, Benjamin, New York, 1968.

The map (x, y) → (2x + y, x + y) mod N is an integer matrix of determinant 1. It is exactly
reversible, preserves area, and is maximally chaotic (Anosov). On an N × N lattice every point
is periodic, so it scrambles and then exactly recurs.
→ use in PWC: (3) and (4). Any pairwise update with a unimodular integer matrix is exactly
invertible on the integers. That is a tool for building reversible laws with no rounding.

### Oseledets (1968): Lyapunov exponents exist
V. I. Oseledets, "Mul'tiplikativnaya ergodicheskaya teorema. Kharakteristicheskie pokazateli
Lyapunova dinamicheskikh sistem", *Trudy Moskovskogo Matematicheskogo Obshchestva* 19 (1968)
179–210; English *Transactions of the Moscow Mathematical Society* 19 (1968) 197–231.

Proves that the Lyapunov exponents of a typical trajectory are well-defined. These measure the
rate at which nearby states separate.
→ use in PWC: measurement tool for all candidates. Report the largest exponent per tick: zero
means crystal-like, large means instant heat death of structure.

### Zakharov & Shabat (1971/72): inverse scattering for the NLS equation
V. E. Zakharov and A. B. Shabat, "Tochnaya teoriya dvumernoi samofokusirovki i odnomernoi
avtomodulyatsii voln v nelineinykh sredakh", *ЖЭТФ* 61 (1971) 118–134; English "Exact theory of
two-dimensional self-focusing and one-dimensional self-modulation of waves in nonlinear media",
*Soviet Physics JETP* 34(1) (1972) 62–69.

Showed that the nonlinear Schrödinger equation is integrable by inverse scattering. Wave
packets form envelope solitons that pass through each other and emerge unchanged except for a
shift. The general theory of integrable waves dates from here.
→ use in PWC: (3). Envelope solitons are the natural "particles" of a wave law: carryable
packets of energy that survive collisions. Basis for the Zone-artifact idea (Part E).

### Kadomtsev & Petviashvili (1970): solitons in two dimensions
B. B. Kadomtsev and V. I. Petviashvili, "Ob ustoichivosti uedinennykh voln v slabo
disperguiruyushchikh sredakh", *ДАН СССР* 192 (1970) 753–756; English "On the stability of
solitary waves in weakly dispersing media", *Soviet Physics Doklady* 15 (1970) 539–541.

A plane KdV soliton is unstable to transverse ripples in one sign of dispersion and stable in
the other. In higher dimensions this decides whether line solitons persist or break into
lumps.
→ use in PWC: (3). Solitons in a 1-D test are not enough; this is the 2-D/3-D stability test.

### Faddeev & Takhtajan; the quantum inverse scattering method
L. D. Faddeev and L. A. Takhtajan, *Gamil'tonov podkhod v teorii solitonov*, Nauka, Moscow,
1986; English *Hamiltonian Methods in the Theory of Solitons*, Springer, Berlin, 1987. Also E. K.
Sklyanin, L. A. Takhtajan and L. D. Faddeev, "Kvantovyi metod obratnoi zadachi. I",
*Teoreticheskaya i Matematicheskaya Fizika* 40(2) (1979) 194–220.

The Leningrad school recast solitons as Hamiltonian systems with action-angle variables and
built the quantum version, the algebraic Bethe ansatz, from which quantum groups grew. Its
central object is the R-matrix that satisfies the Yang–Baxter equation.
→ use in PWC: background for (3) and the Yang–Baxter idea below.

### Zamolodchikov (1980/81): the tetrahedron equation
A. B. Zamolodchikov, "Tetrahedra equations and integrable systems in three-dimensional space",
*ЖЭТФ* 79 (1980) 641–664; English *Soviet Physics JETP* 52 (1980) 325–336.
A. B. Zamolodchikov, "Tetrahedron equations and the relativistic S-matrix of straight-strings in
2+1-dimensions", *Communications in Mathematical Physics* 79 (1981) 489–505.

The Yang–Baxter equation says that three-body scattering in 1+1 dimensions does not depend on
the order of two-body collisions. The tetrahedron equation is its analogue one dimension up,
for straight strings or domain walls in 2+1 dimensions. It is the condition for an exactly
solvable 3-D lattice model.
→ use in PWC: new idea 1. It is the consistency condition a voxel law needs if local updates
should commute in a three-dimensional sense.

### Drinfeld (1992): set-theoretic Yang–Baxter solutions; Veselov's Yang–Baxter maps
V. G. Drinfeld, "On some unsolved problems in quantum group theory", in P. P. Kulish (ed.),
*Quantum Groups (Leningrad, 1990)*, Lecture Notes in Mathematics 1510, Springer, 1992, pp. 1–8.
A. P. Veselov, "Yang–Baxter maps and integrable dynamics", *Physics Letters A* 314 (2003)
214–221. (Veselov citation from memory; title and venue standard, pages not re-checked.)

Drinfeld asked for solutions of the Yang–Baxter equation that are permutations R of X × X for
a finite set X, no linear algebra needed. Such a map is a deterministic, reversible
two-cell collision rule whose three-cell outcomes do not depend on collision order. Veselov
named them Yang–Baxter maps and tied them to integrable discrete dynamics.
→ use in PWC: (4) and new idea 1. A finite-state voxel collision rule that is a Yang–Baxter
map is order-independent, reversible and integer by construction.

---

## Part C. Billiards, entropy, complexity and stable automata

### Sinai (1970): dispersing billiards
Ya. G. Sinai, "Dinamicheskie sistemy s uprugimi otrazheniyami. Ergodicheskie svoistva
rasseivayushchikh bil'yardov", *УМН* 25(2) (1970) 141–192; English "Dynamical systems with elastic
reflections. Ergodic properties of dispersing billiards", *Russian Mathematical Surveys* 25(2)
(1970) 137–189.

A point bouncing among convex scatterers is ergodic and mixing: convex walls spread
neighbouring trajectories apart. The first rigorous proof that a mechanical gas-like system
is chaotic.
→ use in PWC: (4). Fixed convex "obstacle" blocks in a particle CA should thermalize the gas
on their own; flat walls will not.

### Bunimovich & Sinai (1981): diffusion in the Lorentz gas
L. A. Bunimovich and Ya. G. Sinai, "Statistical properties of Lorentz gas with periodic
configuration of scatterers", *Communications in Mathematical Physics* 78 (1981) 479–497.

In a periodic array of scatterers with finite horizon, a particle performs Brownian motion:
the central limit theorem holds and diffusion is normal. Deterministic microscopic motion
gives honest macroscopic diffusion.
→ use in PWC: (4). This is the theorem behind "diffusion with no randomness". Check finite
horizon: if a particle can fly forever along a lattice row, diffusion becomes anomalous.

### Kolmogorov (1958) and Sinai (1959): dynamical entropy
A. N. Kolmogorov, "Novyi metricheskii invariant tranzitivnykh dinamicheskikh sistem i
avtomorfizmov prostranstv Lebega", *ДАН СССР* 119(5) (1958) 861–864.
Ya. G. Sinai, "O ponyatii entropii dinamicheskoi sistemy" [On the notion of entropy of a
dynamical system], *ДАН СССР* 124(4) (1959) 768–771.

The Kolmogorov–Sinai entropy measures how much new information a deterministic system
produces per step. Zero for regular motion, positive for chaos. It is how determinism and an
arrow of time coexist.
→ use in PWC: measurement for (3), (4), (5). Estimate it with compressed snapshot sizes:
"heat" is exactly where per-tick information production is high.

### Kolmogorov (1965): algorithmic complexity
A. N. Kolmogorov, "Tri podkhoda k opredeleniyu ponyatiya 'kolichestvo informatsii'",
*Problemy Peredachi Informatsii* 1(1) (1965) 3–11; English "Three approaches to the
quantitative definition of information", *Problems of Information Transmission* 1 (1965) 1–7.

The information in an object is the length of the shortest program that prints it. A random
string has high complexity; a crystal has low complexity; interesting structure lies between.
→ use in PWC: new idea 4 (complexity-guided worldgen) and a quality metric: score each
candidate's long-run world by compressed size per region, rewarding the middle band.

### Levin (1973): universal search
L. A. Levin, "Universal'nye zadachi perebora" [Universal sequential search problems],
*Problemy Peredachi Informatsii* 9(3) (1973) 115–116. (Standard citation; not re-checked.)

Independently of Cook, introduced NP-completeness, plus an optimal search that runs all
programs in parallel weighted by length. Simple programs are tried first.
→ use in PWC: new idea 4. Enumerate short seeds in order of length and keep the ones that
grow interesting worlds.

### Stavskaya & Pyatetskii-Shapiro (1968): noisy cellular automata
O. N. Stavskaya and I. I. Pyatetskii-Shapiro, "Ob odnorodnykh setyakh spontanno aktivnykh
elementov" [On homogeneous networks of spontaneously active elements], *Problemy Kibernetiki* 20
(1968) 91–106.

One of the first studies of probabilistic cellular automata. A simple 1-D rule shows a phase
transition: below a noise level a state survives, above it the system forgets.
→ use in PWC: background for Toom. The first proof that local rules can hold a memory
against noise, which is what "matter" is.

### Toom (1980): stable cellular automata; Toom's rule
A. L. Toom, "Stable and attractive trajectories in multicomponent systems", in R. L. Dobrushin
and Ya. G. Sinai (eds.), *Multicomponent Random Systems*, Advances in Probability and Related
Topics 6, Marcel Dekker, New York, 1980, pp. 549–575.

Toom's NEC rule: each cell takes the majority of itself, its north and its east neighbour. In
2-D this erodes any finite island of the wrong state, even with small random noise, so two
phases are both stable. The asymmetry is the trick: a symmetric majority vote would let
islands survive.
→ use in PWC: new idea 2 (Toom matter). It is a proof that a local deterministic rule can
store a bit robustly, which is the mathematical definition of a durable solid.

### Gács, Kurdyumov & Levin (1978): the GKL rule
P. Gács, G. L. Kurdyumov and L. A. Levin, "Odnomernye odnorodnye sredy, razmyvayushchie konechnye
ostrova" [One-dimensional uniform arrays that wash out finite islands], *Problemy Peredachi
Informatsii* 14(3) (1978) 92–96.

A 1-D rule in which each cell takes the majority of itself and two cells at distance 1 and 3,
looking left if it is 0 and right if it is 1. Finite islands of the minority are washed out
deterministically. Later famous as a good solver of the density-classification task.
→ use in PWC: new idea 2. A state-dependent look direction is a cheap way to give a
symmetric world a self-healing rule.

### Tsetlin (1961): learning automata
M. L. Tsetlin, "O povedenii konechnykh avtomatov v sluchainykh sredakh", *Avtomatika i
Telemekhanika* 22(10) (1961) 1345–1354; English "On the behavior of finite automata in random
media", *Automation and Remote Control* 22 (1961) 1210–1219. Book: M. L. Tsetlin, *Automaton
Theory and Modeling of Biological Systems*, Academic Press, 1973.

A finite automaton with a "depth of memory" in each action changes action after repeated
penalties. With enough depth it chooses the better action almost always. Collectives of such
automata play games and coordinate with no central control.
→ use in PWC: new idea 3 (learning matter). Gelfand and Tsetlin also wrote the excitable
media paper above; the same group made both ideas.

### Glushkov: abstract automata and cybernetics
V. M. Glushkov, "Abstraktnaya teoriya avtomatov", *УМН* 16(5) (1961) 3–62; English "The abstract
theory of automata", *Russian Mathematical Surveys* 16(5) (1961) (English pages not re-checked). Also V. M. Glushkov,
*Vvedenie v kibernetiku* [Introduction to cybernetics], Kiev, 1964.

Glushkov in Kiev built the algebraic theory of automata (including the Glushkov construction
from regular expressions to automata) and pushed for OGAS, a nationwide computing network for
the economy.
→ use in PWC: tooling, not a law. A cell's rule is an automaton; minimizing it (Glushkov)
gives the true `state_bytes_per_cell`.

### Pontryagin: duality and optimal control
L. S. Pontryagin, "The theory of topological commutative groups", *Annals of Mathematics* 35
(1934) 361–388. L. S. Pontryagin, V. G. Boltyanskii, R. V. Gamkrelidze and E. F. Mishchenko,
*Matematicheskaya teoriya optimal'nykh protsessov*, Fizmatgiz, 1961; English *The Mathematical
Theory of Optimal Processes*, Interscience, 1962.

Pontryagin duality pairs every locally compact abelian group with its character group; Z_N is
self-dual. In lattice gauge theory this underlies electric–magnetic duality. The maximum
principle gives optimal control of dynamical systems.
→ use in PWC: (6). With a Z_N gauge group the law can be dualized: charges and monopoles
swap roles on the dual lattice, giving a free check of correctness.

---

## Part D. Phase transitions, gauge fields and topology

### Landau (1937): order parameters
L. D. Landau, "K teorii fazovykh perekhodov. I", *ЖЭТФ* 7 (1937) 19–32; part II, 627–632. German
"Zur Theorie der Phasenumwandlungen", *Physikalische Zeitschrift der Sowjetunion* 11 (1937) 26–47
and 545–555 (German page ranges not re-checked).

A phase transition is the appearance of an order parameter that breaks a symmetry. Near the
transition, free energy can be written as a power series in the order parameter allowed by
symmetry. This explains why very different systems share critical behaviour.
→ use in PWC: all candidates. Define one order parameter per candidate (crystallinity,
excitation coherence, flux confinement) and look for its transition as a derived constant is
swept; the "phases of matter" should emerge from that, not be listed.

### Ginzburg & Landau (1950); Abrikosov (1957): vortices
V. L. Ginzburg and L. D. Landau, "K teorii sverkhprovodimosti", *ЖЭТФ* 20 (1950) 1064–1082. A. A.
Abrikosov, "O magnitnykh svoistvakh sverkhprovodnikov vtoroi gruppy", *ЖЭТФ* 32 (1957) 1442–1452;
English *Soviet Physics JETP* 5 (1957) 1174–1182. (Standard citations, not re-checked.)

A charged order parameter coupled to a gauge field. In type-II superconductors magnetic flux
enters as quantized vortex tubes that form a lattice.
→ use in PWC: (6). If a matter field is coupled to the lattice gauge field, flux tubes
appear as stable, carryable objects; quantized flux is an exact integer on faces.

### Berezinskii (1971): topological defects in 2-D
V. L. Berezinskii, "Destruction of long-range order in one-dimensional and two-dimensional
systems having a continuous symmetry group. I. Classical systems", *ЖЭТФ* 59 (1970) 907–920;
*Soviet Physics JETP* 32(3) (1971) 493–500.

In 2-D, continuous symmetry cannot break, but vortex–antivortex pairs stay bound at low
temperature and unbind at a transition (later Kosterlitz and Thouless; Nobel 2016 for them).
Topological defects, not local order, carry the phase.
→ use in PWC: (2), (6). Spiral cores in (2) and monopoles in (6) are defects; count them as
the primary observable.

### Polyakov (1974, 1975, 1977): monopoles and confinement in compact QED
A. M. Polyakov, "Particle spectrum in quantum field theory", *Pis'ma v ЖЭТФ* 20 (1974) 430–433;
*JETP Letters* 20 (1974) 194–195. A. M. Polyakov, "Compact gauge fields and the infrared
catastrophe", *Physics Letters B* 59(1) (1975) 82–84, doi:10.1016/0370-2693(75)90162-8. A. M.
Polyakov, "Quark confinement and topology of gauge theories", *Nuclear Physics B* 120(3) (1977)
429–458, doi:10.1016/0550-3213(77)90086-4.

When the gauge field is an angle (compact U(1), or Z_N on a lattice), magnetic monopoles exist
as lattice defects. In 2+1 dimensions a plasma of these instantons confines electric charge:
the force between charges stops falling with distance, a linear string forms. The 1974 paper is
the 't Hooft–Polyakov monopole.
→ use in PWC: (6). Compactness is the switch: integer flux mod N gives monopoles and
confinement, unbounded integer flux gives Coulomb's law. Gameplay: "bound" vs "free" charge
as two phases of the same law.

### Kapitza (1951): dynamic stabilization
P. L. Kapitza, "Dinamicheskaya ustoichivost' mayatnika pri koleblyushcheisya tochke podvesa",
*ЖЭТФ* 21(7) (1951) 588–598 [English "Dynamic stability of a pendulum when its point of
suspension vibrates", in *Collected Papers of P. L. Kapitza*, Pergamon, 1965].

A pendulum whose pivot shakes fast enough stands upright. Fast motion averages into an
effective potential that can turn unstable into stable.
→ use in PWC: (3), (5) and new idea 5. Structures that only stand while they hum.

### Kolmogorov (1941): turbulence cascade
A. N. Kolmogorov, "Lokal'naya struktura turbulentnosti v neszhimaemoi vyazkoi zhidkosti pri
ochen' bol'shikh chislakh Reinol'dsa", *ДАН СССР* 30(4) (1941) 299–303. (Standard citation, not
re-checked.)

Energy injected at large scales cascades to small ones at a constant rate; the spectrum is
E(k) ∝ k^(−5/3). Universal for any fluid.
→ use in PWC: (3), (4). A conserving lattice law should show a cascade from player-made
large disturbances into small-scale "heat". Measure the spectrum.

### Vernadsky: biosphere and noosphere
V. I. Vernadsky, *Biosfera*, Nauchnoe khimiko-tekhnicheskoe izdatel'stvo, Leningrad, 1926;
English *The Biosphere*, Copernicus/Springer, New York, 1998. V. I. Vernadsky, "The biosphere and
the noösphere", *American Scientist* 33(1) (1945) 1–12.

Life is a geological force: "living matter" moves elements through the crust and
atmosphere in cycles driven by sunlight. The noosphere is the next layer, where thought
reshapes the planet.
→ use in PWC: worldgen and (5). Biogeochemical cycles mean elements should flow in closed
loops powered by star light. Vernadsky's framing fits PWC's element-based matter directly.

---

## Part E. Science fiction

### Arkady and Boris Strugatsky, *Roadside Picnic* (1972)
*Piknik na obochine*, serialized in *Avrora* (Leningrad), 1972. English trans. Antonina W. Bouis,
Macmillan, 1977; new trans. Olena Bormashenko, Chicago Review Press, 2012. (Serialization
venue is standard; not re-checked.)

Aliens stopped briefly and left Zones full of artifacts with impossible physics. Examples:
"empties", two copper discs held apart by nothing; one "full empty" with blue liquid trapped
between them; "so-so" (or "batteries"), black rods of endless energy that multiply by
division; "black sprays", whose purpose nobody knows; "witches' jelly" (hell slime), which
seeps through anything but porcelain; "mosquito mange", spots of crushing gravity.
→ use in PWC: Zone artifacts as stable field configurations of the law itself: a soliton or
breather in (3) is an "empty", a trapped flux loop in (6) is a "full empty", a self-sustaining
spiral core in (2) is a "so-so". Players find, carry and place them; nothing is scripted.

### Strugatsky, *Monday Begins on Saturday* (1965)
*Ponedel'nik nachinaetsya v subbotu*, Detskaya Literatura, Moscow, 1965. English trans. Leonid
Renen, DAW, 1977.

NIICHAVO, a research institute of magic run like a Soviet lab. Director Janus Poluektovich is
two people: A-Janus and U-Janus, who lives backwards in time by "countermotion".
→ use in PWC: (3) and (4). Reversible laws let a "countermoving" object exist: run a region's
own time backwards (exact inverse step) as an artifact or player tool, possible only if the
law is exactly reversible.

### Strugatsky, *Far Rainbow* (1963) and the Noon Universe
*Dalekaya Raduga*, 1963. The Noon Universe also includes *Noon: 22nd Century* (1962), *Hard to
Be a God* (1964) and *Beetle in the Anthill* (1979).

On the science planet Rainbow, zero-transportation experiments release the Wave, a front of
destruction that sweeps in from both poles. The Noon books also feature the Wanderers, an
older civilization whose traces are everywhere and whose aims are unreadable.
→ use in PWC: (2). The Wave is a planet-scale excitable front triggered by a player
experiment; a world where large fronts are possible but rare is a good drama generator.

### Stanisław Lem, *Solaris* (1961)
*Solaris*, Wydawnictwo MON, Warsaw, 1961. English (via French) Faber/Walker, 1970; direct trans.
Bill Johnston, audiobook 2011.

A planetary ocean makes huge transient structures that scientists catalogue in vain:
mimoids (copies of nearby objects), symmetriads (vast symmetric forms that build and collapse),
asymmetriads, vertebrids, extensors. Their behaviour is lawful but the law is never found.
→ use in PWC: (2)/(5) "ocean" biome. A large excitable region seeded with matter readings that
mimics nearby shapes (mimoid = wavefronts reflecting the outline of placed blocks) and forms
transient symmetric structures from pacemakers.

### Lem, *The Invincible* (1964)
*Niezwyciężony*, Wydawnictwo MON, 1964. English Seabury, 1973; trans. Bill Johnston, 2006 (later
editions).

On Regis III, self-replicating machines evolved without life ("necroevolution") into tiny
flying units that swarm into black clouds and wipe memory. The best survivor is not the
smartest but the simplest.
→ use in PWC: a long-run test for (4) and (5). If a law can sustain replicators, they will be
small and numerous. A "cloud" that erases information (resets cells to rest) is a natural
hazard of a reversible-plus-drive law.

### Lem, *Summa Technologiae* (1964)
*Summa Technologiae*, Wydawnictwo Literackie, Kraków, 1964. English trans. Joanna Zylinska,
University of Minnesota Press, 2013.

A book of essays on future technology. "Pantocreatics" is Lem's name for the ability to
achieve any goal, including those Nature never realized, "anchored in the general theory of
physical and mathematical systems". "Imitology" is the science of imitating natural processes;
"phantomatics" is virtual reality; the chapter "The Creation of Worlds" covers information
farming and cosmogonic engineering.
→ use in PWC: framing. PWC is a pantocreatic engine: one law, any outcome reachable. Lem's
"information farming" (grow knowledge instead of designing it) is how worldgen should work.

### Lem, *The Cyberiad* (1965) and *A Perfect Vacuum* (1971)
*Cyberiada*, Wydawnictwo Literackie, 1965; English trans. Michael Kandel, Seabury, 1974.
*Doskonała próżnia*, Czytelnik, 1971; English trans. Michael Kandel, Harcourt, 1979.

In *The Cyberiad* two robot constructors build machines that can make anything, poems from
first principles, and a boxed kingdom of simulated people (the "Seventh Sally"). In *A Perfect
Vacuum*, "The New Cosmogony" proposes that the laws of physics are the current state of a game
played by ancient civilizations; "Non Serviam" describes simulated beings ("personoids") who
argue about their creator.
→ use in PWC: The New Cosmogony is the brief's philosophy in fiction. Possible mechanic: law
constants are player-negotiated slowly over server lifetime ("the game of the laws").

### Ivan Yefremov, *Andromeda Nebula* (1957)
*Tumannost' Andromedy*, Molodaya Gvardiya, Moscow, 1957. English *Andromeda: A Space-Age
Tale*, Foreign Languages Publishing House, Moscow, 1959.

A far-future communist Earth belongs to the Great Ring, a network of civilizations exchanging
knowledge by broadcasts that take centuries. A key experiment tries to cross space through
"zero-space" and goes wrong.
→ use in PWC: multiplayer. The Ring is a model for servers exchanging discovered structures
(artifacts, recipes) as data, a slow galaxy-wide catalogue of what the law can do.

### Kir Bulychev: Veliky Guslar and Alisa
Kir Bulychev (Igor Mozheiko). The Veliky Guslar cycle began with "Svyazi lichnogo kharaktera"
in *Znanie–Sila*, 1970 (English selection *Gusliar Wonders*, Macmillan, 1983). The Alisa
Selezneva books began with *Devochka, s kotoroi nichego ne sluchitsya*, 1965 (date not re-checked).

In a sleepy provincial town, miracles and aliens are daily nuisances solved by neighbours and
the absent-minded Professor Lev Khristoforovich Minz. Wonder is local and domestic.
→ use in PWC: tone. Emergent oddities should appear in small player bases too, not only at
the far edge of the map.

### Alexander Belyaev
*Golova professora Douelya* (1925), *Chelovek-amfibiya* (1928), *Zvezda KETs* (1936), *Ariel'*
(1941).

The "Soviet Jules Verne". *Amphibian Man* gives a boy shark gills; *KETs Star* (initials of
Tsiolkovsky) describes an orbital station; *Ariel* is a boy taught to fly by a lab experiment.
→ use in PWC: player tools that change which law-quantities the avatar couples to (e.g. an
"Ariel" suit that cancels the player's coupling to gravity flux).

### Yevgeny Zamyatin, *We* (1920–21)
*My*, written 1920–21; first published in English (trans. Gregory Zilboorg), Dutton, New York,
1924. Essay "O literature, revolyutsii, entropii i o prochem" [On literature, revolution,
entropy and other matters], 1923, printed 1924.

The One State is ruled by mathematics behind a Green Wall; the hero is shaken by √−1, an
"irrational" feeling. In the essay Zamyatin sets energy (revolution, heresy) against entropy
(dogma) and says there is no final revolution.
→ use in PWC: design principle. A world that reaches equilibrium is dead; the law must keep
an energy source that undoes entropy locally (stars, metastable fuel).

### Konstantin Tsiolkovsky and Russian cosmism
K. E. Tsiolkovsky, "Issledovanie mirovykh prostranstv reaktivnymi priborami" [Exploration of
cosmic space by means of reaction devices], *Nauchnoe Obozrenie* no. 5, 1903. K. E. Tsiolkovsky,
*Monizm vselennoi* [Monism of the universe], self-published, Kaluga, 1925.

The rocket equation, and a philosophy: all matter is one substance obeying one set of laws,
and every atom has a seed of sensation. Humans must spread into space; the cosmos is meant to
be inhabited.
→ use in PWC: the monism is the brief's rule "no material is named": one substance, one
law. Space-campaign hook: rocket equation emerges if propellant is conserved stuff.

### Anatoly Dneprov, "Crabs on the Island" (1958)
"Kraby idut po ostrovu", 1958. English in Soviet SF anthologies (e.g. Mir Publishers).
(Original venue not re-checked.)

Engineers release self-copying metal crabs on an island to breed better war machines by
selection. They eat all the metal, then each other, and evolution ends in one giant crab that
cannot reproduce for lack of energy.
→ use in PWC: Gause in fiction. Replicators need renewable resources; the law's resource
budget decides whether evolution keeps going or ends in one giant crab.

---

## Five ideas no candidate covers yet

### 1. A Yang–Baxter law: reactions whose result does not depend on scheduling
Build the law as a set-theoretic Yang–Baxter map (Drinfeld 1992), lifted to 3-D with the
tetrahedron equation (Zamolodchikov 1980) as the check. Each face update is a permutation of
(state_a, state_b) pairs, so it is reversible and integer. Because the Yang–Baxter condition
holds, the outcome of a region does not depend on the order in which faces are processed.
This removes the need for a fixed global update order, so chunks, threads and peers can update
in any order and still produce the same bytes. Start in 1-D and 2-D, search small state sets
(|X| ≤ 16) by computer for maps that satisfy the equations and are non-trivial.

### 2. Toom matter: solids as error-correcting phases
Make the durability of solids an error-correcting process instead of a bond strength. Each
material phase is held by a Toom-like asymmetric majority rule (Toom 1980; GKL 1978): small
damage heals on its own, damage larger than a threshold spreads. Breaking becomes a real
nucleation event, cracks have a critical length, and tools work by making islands bigger than
the healing radius. The eroder's preferred direction can be set by the local gravity or heat
gradient, so the asymmetry is physical, not arbitrary. This gives "matter remembers its shape"
without a Lyapunov descent.

### 3. Learning matter: Tsetlin automata in every cell
Give each cell a few bits of Tsetlin-style memory depth: it repeats the last response while it
is "rewarded" (energy flowed through it well) and switches after repeated "penalties"
(Tsetlin 1961). Rewards and penalties come from the existing law, not a script. Paths that
carry flux get easier to use, like trails worn into grass, water carving channels or neurons
strengthening. The Gelfand–Tsetlin group built both excitable media and learning automata, and
joining them gives media that change their own thresholds. Everything stays deterministic and
integer, and the extra state is 2–4 bits per cell.

### 4. Complexity-guided worldgen ("The New Cosmogony")
Generate worlds by running the law itself from short seeds, then keep those whose long-run
state has middle complexity: compressed size far from both the crystal and the noise limit
(Kolmogorov 1965). Enumerate seeds shortest first (Levin 1973), so the world comes from the
simplest program that produces something interesting. Terrain, ores and biomes then follow the
same physics the player meets. This is Lem's "information farming" and his "New Cosmogony"
made literal: the laws plus the seed are the whole world, with no separate generator to drift
from the law.

### 5. Kapitza architecture: structures that stand only while they hum
In a conserving vibration law (3 or 5), fast oscillation creates an effective potential that
can stabilize otherwise unstable configurations (Kapitza 1951). So some structures, such as
overhangs, floating blocks and inverted towers, would stand only while energy circulates
through them at the right frequency, and would fall when the drive stops. This gives a physical
basis for machines, shields and the Strugatsky "empties" (two plates held apart by nothing but
a standing wave). It needs no new quantity, only a check that the law's fast-mode averaging is
honest. It would make "power" a visible, emergent resource.
