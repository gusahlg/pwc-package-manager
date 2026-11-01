// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local finite reservoirs and conservative matter rearrangement. See ../README.md.
use pwc_module_api::Action;
use pwc_module_api::engine::Key;
use pwc_module_api::input::intent::{Chord, Modules, Source};
use pwc_module_api::material::{Block, CAPACITY, Configuration, QUANTUM, fit_raw};
use pwc_module_api::prelude::*;
use pwc_module_api::sim::{model::*, reactions::*};
use std::collections::{BTreeMap, BTreeSet};

const ID: &str = "pwc.physics-reservoir";
const MAX_ENERGY_CELLS: usize = 8192;
const MAX_CONTACTS: usize = 16_384;
const MAX_TOTAL_ENERGY: i64 = 1 << 60;

pub fn register(registrar: &mut ModuleRegistrar) {
    let enabled = registrar.option(OptionSpec::toggle(
        "enabled",
        "Conservative Reservoir Physics",
        Category::World,
        false,
    ));
    registrar.add(ReservoirModule { enabled, on: false });
}

pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Reservoir::default()),
    }
}
pub struct ReservoirModule {
    enabled: OptionId,
    on: bool,
}
impl Module for ReservoirModule {
    fn id(&self) -> &'static str {
        ID
    }
    fn on_options(&mut self, options: &Options) {
        self.on = options.bool(self.enabled);
    }
    fn reaction_model(&self) -> Option<ModelSpec> {
        self.on.then_some(spec())
    }
    fn actions(&self) -> &[Action] {
        const ACTIONS: &[Action] = &[Action {
            id: "physics.reservoir.heat",
            label: "Reservoir laboratory energy input",
            default: &[Chord::with(Modules::CTRL, Source::Key(Key::F3))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        ACTIONS
    }
    fn update(&mut self, ctx: &mut ModuleContext) {
        if !ctx.networked
            && ctx.world.simulation_model_id() == Some(ID)
            && ctx.action("physics.reservoir.heat")
        {
            let pos = ctx.world.simulation_cell(ctx.player.position);
            ctx.world.perturb_simulation(pos, 4 * i64::from(QUANTUM));
        }
    }
    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if world.simulation_model_id() != Some(ID) || !facts.hud_mode.shows_module_hud() {
            return;
        }
        let s = world.simulation_sample(world.simulation_cell(player.position));
        let stats = world.simulation_stats();
        out.push(HudElement::Label {
            at: Anchor::BottomLeft,
            off: (12, -82),
            base_fs: 15,
            role: Role::Muted,
            text: format!(
                "Reservoir local={} total={} contacts={} changes={} | Ctrl+F3 input",
                s.values[0], s.values[1], stats.pending, stats.operations
            )
            .into(),
        });
    }
}

#[derive(Default)]
pub struct Reservoir {
    energy: BTreeMap<Pos, i64>,
    total: i64,
    queue: BTreeSet<(u64, Contact)>,
    active: BTreeSet<Contact>,
    turns: u64,
    operations: u64,
    dropped: u64,
}

fn around(pos: Pos) -> Vec<Contact> {
    let mut out = Vec::with_capacity(6);
    for axis in 0..3 {
        let coordinate = match axis {
            0 => pos.0,
            1 => pos.1,
            _ => pos.2,
        };
        if coordinate < i32::MAX {
            out.push(Contact { lo: pos, axis });
        }
        if coordinate > i32::MIN {
            let lo = match axis {
                0 => (pos.0 - 1, pos.1, pos.2),
                1 => (pos.0, pos.1 - 1, pos.2),
                _ => (pos.0, pos.1, pos.2 - 1),
            };
            out.push(Contact { lo, axis });
        }
    }
    out
}

/// Scan affordable *single-occurrence transfers* from a rotating canonical starting point.
/// Capacity excludes insertion; swaps are deliberately not an additional law in this model.
fn proposal(
    a: &Block,
    b: &Block,
    energy: i64,
    phase: u64,
) -> Option<(Configuration, Configuration, i64)> {
    for side in 0..2 {
        let from_a = ((phase & 1) as usize ^ side) == 0;
        let (donor, receiver) = if from_a { (a, b) } else { (b, a) };
        if donor.is_empty() || receiver.len() == CAPACITY {
            continue;
        }
        let start = ((phase / 2) % donor.len() as u64) as usize;
        for delta in 0..donor.len() {
            let i = (start + delta) % donor.len();
            let element = donor.elements()[i];
            let gain = receiver
                .elements()
                .iter()
                .map(|&e| i64::from(fit_raw(element, e)))
                .sum::<i64>()
                - i64::from(donor.holding()[i]);
            if energy
                .checked_add(gain)
                .is_none_or(|pool| pool < 0 || pool > MAX_TOTAL_ENERGY)
            {
                continue;
            }
            let mut d = donor.elements().to_vec();
            d.remove(i);
            let mut r = receiver.elements().to_vec();
            r.push(element);
            let d = Configuration::new(d).expect("removal respects capacity");
            let r = Configuration::new(r).expect("insertion was capacity checked");
            return Some(if from_a { (d, r, gain) } else { (r, d, gain) });
        }
    }
    None
}

impl Reservoir {
    fn enqueue(&mut self, arrival: u64, c: Contact) {
        if self.active.contains(&c) {
            return;
        }
        if self.active.len() >= MAX_CONTACTS {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.active.insert(c);
        self.queue.insert((arrival, c));
    }
    fn wake_at(&mut self, turn: u64, pos: Pos) {
        for c in around(pos) {
            self.enqueue(turn, c);
        }
    }
    fn at(&self, p: Pos) -> i64 {
        self.energy.get(&p).copied().unwrap_or(0)
    }
    fn fits(&self, a: Pos, ea: i64, b: Pos, eb: i64) -> bool {
        let mut count = self.energy.len();
        for (p, e) in [(a, ea), (b, eb)] {
            if e > 0 && !self.energy.contains_key(&p) {
                count += 1;
            }
            if e == 0 && self.energy.contains_key(&p) {
                count -= 1;
            }
        }
        count <= MAX_ENERGY_CELLS
    }
    fn put(&mut self, p: Pos, energy: i64) {
        if energy == 0 {
            self.energy.remove(&p);
        } else {
            self.energy.insert(p, energy);
        }
    }
}

impl Model for Reservoir {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, pos: Pos) {
        self.wake_at(self.turns, pos);
    }
    fn perturb(&mut self, pos: Pos, amount: i64) {
        let Some(energy) = self.at(pos).checked_add(amount) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        let Some(total) = self.total.checked_add(amount) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        if energy < 0
            || !(0..=MAX_TOTAL_ENERGY).contains(&total)
            || (energy > 0
                && !self.energy.contains_key(&pos)
                && self.energy.len() == MAX_ENERGY_CELLS)
        {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.put(pos, energy);
        self.total = total;
        self.wake_cell(pos);
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, out: &mut Vec<Mutation>) {
        let Some(next_turn) = self.turns.checked_add(1) else {
            self.dropped = self.dropped.saturating_add(1);
            return;
        };
        for _ in 0..budget.contacts_per_turn {
            let Some(&(arrival, c)) = self.queue.first() else {
                break;
            };
            if arrival > self.turns {
                break;
            }
            self.queue.pop_first();
            self.active.remove(&c);
            let (lo, hi) = (store.resolve(c.lo), store.resolve(c.hi()));
            if lo == hi {
                continue;
            }
            let (Some(a), Some(b)) = (store.block_at(lo), store.block_at(hi)) else {
                continue;
            };
            let pool = self.at(lo) + self.at(hi); // Total energy is bounded by 2^60.
            let (ea, eb) = ((pool + 1) / 2, pool / 2);
            let mut changed = false;
            let mut refused = false;
            if ea != self.at(lo) || eb != self.at(hi) {
                if self.fits(lo, ea, hi, eb) {
                    self.put(lo, ea);
                    self.put(hi, eb);
                    changed = true;
                    self.operations = self.operations.saturating_add(1);
                } else {
                    self.dropped = self.dropped.saturating_add(1);
                    refused = true;
                }
            }
            let phase = self
                .turns
                .wrapping_add(c.lo.0 as u64)
                .wrapping_add((c.lo.1 as u64).rotate_left(21))
                .wrapping_add((c.lo.2 as u64).rotate_left(42))
                .wrapping_add(u64::from(c.axis));
            let candidate = proposal(
                store.registry().block(a),
                store.registry().block(b),
                pool,
                phase,
            );
            if let Some((na, nb, gain)) = candidate {
                let total = self.total + gain;
                let pool = pool + gain;
                let (ea, eb) = ((pool + 1) / 2, pool / 2);
                if !(0..=MAX_TOTAL_ENERGY).contains(&total) || !self.fits(lo, ea, hi, eb) {
                    self.dropped = self.dropped.saturating_add(1);
                    refused = true;
                } else if let (Some(na), Some(nb)) = (
                    store.registry_mut().intern(&na),
                    store.registry_mut().intern(&nb),
                ) {
                    if store.set_pair(lo, a, na, hi, b, nb) {
                        self.put(lo, ea);
                        self.put(hi, eb);
                        self.total = total;
                        out.push(Mutation {
                            pos: lo,
                            from: a,
                            to: na,
                        });
                        out.push(Mutation {
                            pos: hi,
                            from: b,
                            to: nb,
                        });
                        changed = true;
                        self.operations = self.operations.saturating_add(1);
                    } else {
                        self.dropped = self.dropped.saturating_add(1);
                        refused = true;
                    }
                } else {
                    self.dropped = self.dropped.saturating_add(1);
                    refused = true;
                }
            }
            if changed {
                self.wake_at(next_turn, lo);
                self.wake_at(next_turn, hi);
            }
            if refused {
                self.enqueue(next_turn, c);
            }
        }
        self.turns = next_turn;
    }
    fn sample(&self, pos: Pos) -> Sample {
        Sample {
            labels: [
                "local reservoir",
                "total reservoir",
                "reservoir cells",
                "pending contacts",
            ],
            values: [
                self.at(pos),
                self.total,
                self.energy.len() as i64,
                self.active.len() as i64,
            ],
        }
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.dropped,
            pending: self.active.len(),
        }
    }
    fn save(&self) -> String {
        let mut s = format!(
            "reservoir1 {} {} {}\n",
            self.turns, self.operations, self.dropped
        );
        for (&(x, y, z), &e) in &self.energy {
            s.push_str(&format!("e {x} {y} {z} {e}\n"));
        }
        for &(arrival, c) in &self.queue {
            s.push_str(&format!(
                "q {arrival} {} {} {} {}\n",
                c.lo.0, c.lo.1, c.lo.2, c.axis
            ));
        }
        s
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let h: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
        if h.len() != 4 || h[0] != "reservoir1" {
            return Err("unsupported reservoir state".into());
        }
        let counter = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "bad reservoir counter".to_owned())
        };
        let mut next = Reservoir {
            turns: counter(h[1])?,
            operations: counter(h[2])?,
            dropped: counter(h[3])?,
            ..Self::default()
        };
        if next.turns == u64::MAX {
            return Err("exhausted reservoir clock".into());
        }
        let coord = |s: &str| {
            s.parse::<i32>()
                .map_err(|_| "bad reservoir position".to_owned())
        };
        for line in lines {
            let v: Vec<_> = line.split_whitespace().collect();
            match v.first().copied() {
                Some("e") if v.len() == 5 => {
                    if next.energy.len() >= MAX_ENERGY_CELLS {
                        return Err("reservoir cells exceed capacity".into());
                    }
                    let e = v[4]
                        .parse::<i64>()
                        .map_err(|_| "bad reservoir energy".to_owned())?;
                    if !(1..=MAX_TOTAL_ENERGY).contains(&e) {
                        return Err("invalid reservoir energy".into());
                    }
                    next.total = next
                        .total
                        .checked_add(e)
                        .filter(|&n| n <= MAX_TOTAL_ENERGY)
                        .ok_or("reservoir total exceeds capacity")?;
                    if next
                        .energy
                        .insert((coord(v[1])?, coord(v[2])?, coord(v[3])?), e)
                        .is_some()
                    {
                        return Err("duplicate reservoir cell".into());
                    }
                }
                Some("q") if v.len() == 6 => {
                    if next.active.len() >= MAX_CONTACTS {
                        return Err("reservoir contacts exceed capacity".into());
                    }
                    let arrival = counter(v[1])?;
                    if arrival > next.turns + 1 {
                        return Err("reservoir arrival exceeds the next turn".into());
                    }
                    let lo = (coord(v[2])?, coord(v[3])?, coord(v[4])?);
                    let axis = v[5]
                        .parse::<u8>()
                        .map_err(|_| "bad reservoir axis".to_owned())?;
                    if axis > 2
                        || match axis {
                            0 => lo.0,
                            1 => lo.1,
                            _ => lo.2,
                        } == i32::MAX
                    {
                        return Err("invalid reservoir contact".into());
                    }
                    let c = Contact { lo, axis };
                    if !next.active.insert(c) {
                        return Err("duplicate reservoir contact".into());
                    }
                    next.queue.insert((arrival, c));
                }
                _ => return Err("bad reservoir row".into()),
            }
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_module_api::material::Element;
    #[test]
    fn uphill_transfer_borrows_energy_and_conserves_occurrences_and_hamiltonian() {
        let a = Block::new(&[Element([0; 4]), Element([8; 4])]).unwrap();
        let b = Block::default();
        let binding = a.internal_fit();
        assert!(binding > 0, "this fixture is a cohesive pair");
        assert!(proposal(&a, &b, 0, 0).is_none());
        let initial = binding + 17;
        let (na, nb, gain) =
            proposal(&a, &b, initial, 0).expect("stored energy pays for separation");
        assert!(gain < 0);
        assert_eq!(na.len() + nb.len(), a.len() + b.len());
        let before = initial - a.internal_fit() - b.internal_fit();
        let after = initial + gain - Block::of(&na).internal_fit() - Block::of(&nb).internal_fit();
        assert_eq!(before, after);
        let mut occurrences = na.elements().to_vec();
        occurrences.extend(nb.elements());
        occurrences.sort();
        let mut original = a.elements().to_vec();
        original.sort();
        assert_eq!(occurrences, original);
        println!(
            "uphill gain={gain}, reservoir before={initial}, after={}, conserved E={after}",
            initial + gain
        );
    }
}
