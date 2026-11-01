// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Cooperative compatibility ascent: allow the smallest cluster beyond one occurrence.
use pwc_mod_api::material::{CAPACITY, Configuration, fit_raw};
use pwc_mod_api::prelude::*;
use pwc_mod_api::sim::{
    model::{Model, ModelSpec, Sample, Stats},
    reactions::{Budget, CellStore, Contact, Mutation, Pos},
};
use pwc_mod_api::{
    Action,
    engine::Key,
    input::intent::{Chord, Mods, Source},
};
use std::collections::BTreeSet;

const ID: &str = "pwc.physics-cooperative";
const LIMIT: usize = 16384;

/// Canonical package entry point. This experiment is disabled until explicitly selected.
pub fn register(r: &mut ModRegistrar) {
    let option = r.option(OptionSpec::toggle(
        "enabled",
        "Cooperative Chemistry",
        Category::World,
        false,
    ));
    r.add(Provider {
        option,
        enabled: false,
    });
}
/// Native authority factory; version one defines this law and payload.
pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Cooperative::default()),
    }
}
struct Provider {
    option: OptionId,
    enabled: bool,
}
impl Mod for Provider {
    fn id(&self) -> &'static str {
        ID
    }
    fn on_options(&mut self, o: &Options) {
        self.enabled = o.bool(self.option);
    }
    fn reaction_model(&self) -> Option<ModelSpec> {
        self.enabled.then_some(spec())
    }
    fn actions(&self) -> &[Action] {
        const A: &[Action] = &[Action {
            id: "physics.cooperative.wake",
            label: "Wake cooperative chemistry",
            default: &[Chord::with(Mods::CTRL, Source::Key(Key::F5))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        A
    }
    fn update(&mut self, c: &mut ModContext) {
        if !c.networked
            && c.world.simulation_model_id() == Some(ID)
            && c.action("physics.cooperative.wake")
        {
            let p = c.world.simulation_cell(c.player.position);
            c.world.perturb_simulation(p, 1);
        }
    }
    fn hud(&self, f: &HudFacts, w: &World, _: &Player, out: &mut Vec<HudElement>) {
        if w.simulation_model_id() == Some(ID) && f.hud_mode.shows_mod_hud() {
            let s = w.simulation_sample((0, 0, 0));
            out.push(HudElement::Label {
                at: Anchor::BottomLeft,
                off: (12, -82),
                base_fs: 15,
                role: Role::Muted,
                text: format!(
                    "Cooperative contacts={} changes={} double transfers={} | Ctrl+F5 wake",
                    s.values[0], s.values[1], s.values[2]
                )
                .into(),
            });
        }
    }
}

fn safe(p: Pos) -> bool {
    [p.0, p.1, p.2]
        .into_iter()
        .all(|v| v.unsigned_abs() <= 2_000_000_000)
}
#[cfg(test)]
fn potential(c: &Configuration) -> i64 {
    let e = c.elements();
    (0..e.len())
        .flat_map(|i| (i + 1..e.len()).map(move |j| i64::from(fit_raw(e[i], e[j]))))
        .sum()
}

/// Search all one- and two-occurrence removals. Cross/internal supports are computed once,
/// so both candidate families together use O(C²) fits rather than rebuilding every product.
fn proposal(
    a: &Configuration,
    b: &Configuration,
) -> Option<(Configuration, Configuration, usize, i64)> {
    let mut best: Option<(bool, usize, Option<usize>, i64)> = None;
    for from_a in [true, false] {
        let (donor, receiver) = if from_a { (a, b) } else { (b, a) };
        let d = donor.elements();
        let gain: Vec<i64> = d
            .iter()
            .enumerate()
            .map(|(i, &e)| {
                receiver
                    .elements()
                    .iter()
                    .map(|&r| i64::from(fit_raw(e, r)))
                    .sum::<i64>()
                    - d.iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, &v)| i64::from(fit_raw(e, v)))
                        .sum::<i64>()
            })
            .collect();
        for i in 0..d.len() {
            if receiver.len() < CAPACITY && gain[i] > best.as_ref().map_or(0, |v| v.3) {
                best = Some((from_a, i, None, gain[i]));
            }
            if receiver.len() + 2 > CAPACITY {
                continue;
            }
            for j in i + 1..d.len() {
                let g = gain[i] + gain[j] + 2 * i64::from(fit_raw(d[i], d[j]));
                if g > best.as_ref().map_or(0, |v| v.3) {
                    best = Some((from_a, i, Some(j), g));
                }
            }
        }
    }
    let (from_a, i, j, g) = best?;
    let (donor, receiver) = if from_a { (a, b) } else { (b, a) };
    let mut d = donor.elements().to_vec();
    let mut r = receiver.elements().to_vec();
    if let Some(j) = j {
        r.push(d.remove(j));
    }
    r.push(d.remove(i));
    let n = if j.is_some() { 2 } else { 1 };
    let d = Configuration::new(d).ok()?;
    let r = Configuration::new(r).ok()?;
    Some(if from_a { (d, r, n, g) } else { (r, d, n, g) })
}

#[derive(Default)]
struct Cooperative {
    queue: BTreeSet<(u64, Contact)>,
    active: BTreeSet<Contact>,
    turns: u64,
    operations: u64,
    dropped: u64,
    doubles: u64,
}
impl Cooperative {
    fn enqueue(&mut self, t: u64, c: Contact) {
        if !safe(c.lo) || self.active.contains(&c) {
            return;
        }
        if self.active.len() == LIMIT {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.active.insert(c);
        self.queue.insert((t, c));
    }
    fn wake(&mut self, t: u64, p: Pos) {
        if safe(p) {
            for c in Contact::around(p) {
                self.enqueue(t, c);
            }
        }
    }
}
impl Model for Cooperative {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, p: Pos) {
        self.wake(self.turns, p);
    }
    fn tick(&mut self, s: &mut dyn CellStore, budget: Budget, out: &mut Vec<Mutation>) {
        let Some(next) = self.turns.checked_add(1) else {
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
            let (a_pos, b_pos) = (s.resolve(c.lo), s.resolve(c.hi()));
            // Raw chart contacts need a shared port map before this can be a physical seam law.
            if a_pos != c.lo || b_pos != c.hi() || a_pos == b_pos {
                self.dropped = self.dropped.saturating_add(1);
                continue;
            }
            let (Some(a), Some(b)) = (s.block_at(a_pos), s.block_at(b_pos)) else {
                continue;
            };
            let Some((na, nb, n, _)) =
                proposal(s.registry().configuration(a), s.registry().configuration(b))
            else {
                continue;
            };
            let (na, nb) = {
                let r = s.registry_mut();
                (r.intern(&na), r.intern(&nb))
            };
            let (Some(na), Some(nb)) = (na, nb) else {
                self.enqueue(next, c);
                self.dropped = self.dropped.saturating_add(1);
                continue;
            };
            if !s.set_pair(a_pos, a, na, b_pos, b, nb) {
                self.enqueue(next, c);
                self.dropped = self.dropped.saturating_add(1);
                continue;
            }
            out.push(Mutation {
                pos: a_pos,
                from: a,
                to: na,
            });
            out.push(Mutation {
                pos: b_pos,
                from: b,
                to: nb,
            });
            self.operations = self.operations.saturating_add(1);
            self.doubles = self.doubles.saturating_add(u64::from(n == 2));
            self.wake(next, a_pos);
            self.wake(next, b_pos);
        }
        self.turns = next;
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.dropped,
            pending: self.active.len(),
        }
    }
    fn sample(&self, _: Pos) -> Sample {
        Sample {
            labels: ["contacts", "changes", "double transfers", "turns"],
            values: [
                self.active.len() as i64,
                self.operations as i64,
                self.doubles as i64,
                self.turns as i64,
            ],
        }
    }
    fn save(&self) -> String {
        let mut t = format!(
            "cooperative1 {} {} {} {}\n",
            self.turns, self.operations, self.dropped, self.doubles
        );
        for &(arrival, c) in &self.queue {
            t.push_str(&format!(
                "{arrival} {} {} {} {}\n",
                c.lo.0, c.lo.1, c.lo.2, c.axis
            ));
        }
        t
    }
    fn load(&mut self, t: &str) -> Result<(), String> {
        let mut lines = t.lines();
        let h: Vec<_> = lines
            .next()
            .ok_or("missing cooperative header")?
            .split_whitespace()
            .collect();
        if h.len() != 5 || h[0] != "cooperative1" {
            return Err("invalid cooperative header".into());
        }
        let count = |v: &str| {
            v.parse::<u64>()
                .map_err(|_| "invalid cooperative count".to_string())
        };
        let mut n = Self {
            turns: count(h[1])?,
            operations: count(h[2])?,
            dropped: count(h[3])?,
            doubles: count(h[4])?,
            ..Default::default()
        };
        if n.turns == u64::MAX || n.doubles > n.operations {
            return Err("invalid cooperative counters".into());
        }
        for line in lines {
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 5 || n.queue.len() == LIMIT {
                return Err("invalid cooperative contact".into());
            }
            let arrival = count(v[0])?;
            let coord = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "invalid cooperative position".to_string())
            };
            let p = (coord(v[1])?, coord(v[2])?, coord(v[3])?);
            let axis = v[4].parse::<u8>().map_err(|_| "invalid cooperative axis")?;
            if !safe(p) || axis > 2 || arrival > n.turns + 1 {
                return Err("invalid cooperative contact range".into());
            }
            let c = Contact { lo: p, axis };
            if !n.active.insert(c) {
                return Err("duplicate cooperative contact".into());
            }
            n.queue.insert((arrival, c));
        }
        *self = n;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::material::Element;
    #[test]
    fn cluster_move_changes_matter_without_fabricating_occurrences() {
        let a = Configuration::new(vec![Element::new([0, 0, 0, 0]), Element::new([1, 1, 1, 1])])
            .unwrap();
        let b = Configuration::void();
        // Across a vacuum, moving the cohesive pair is energy-neutral and must be refused.
        assert!(potential(&a) > 0);
        assert!(proposal(&a, &b).is_none());
        // A true local trap in both directions. Parallel cohesive pairs have a mildly
        // negative offset in the second resource axis: every single gain is 2*J(88)<0,
        // while moving a complete pair gains 2*J(64)+4*J(88)>0.
        let a = Configuration::new(vec![
            Element::new([0, 0, 0, 0]),
            Element::new([64, 0, 0, 0]),
        ])
        .unwrap();
        let b = Configuration::new(vec![
            Element::new([0, 88, 0, 0]),
            Element::new([64, 88, 0, 0]),
        ])
        .unwrap();
        for (d, r) in [(&a, &b), (&b, &a)] {
            for (i, &e) in d.elements().iter().enumerate() {
                let gain = r
                    .elements()
                    .iter()
                    .map(|&v| i64::from(fit_raw(e, v)))
                    .sum::<i64>()
                    - d.elements()
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, &v)| i64::from(fit_raw(e, v)))
                        .sum::<i64>();
                assert!(
                    gain < 0,
                    "every single transfer in both orientations loses fit"
                );
            }
        }
        let (na, nb, n, g) =
            proposal(&a, &b).expect("cooperative transfer crosses the one-occurrence trap");
        assert_eq!(n, 2);
        let mut before = a
            .elements()
            .iter()
            .chain(b.elements())
            .copied()
            .collect::<Vec<_>>();
        before.sort();
        let mut after = na
            .elements()
            .iter()
            .chain(nb.elements())
            .copied()
            .collect::<Vec<_>>();
        after.sort();
        assert_eq!(before, after);
        assert_eq!(
            potential(&na) + potential(&nb) - potential(&a) - potential(&b),
            g
        );
        assert!(g > 0);
        println!(
            "two-occurrence preference found: gain={g}; exact occurrence multiset and potential delta"
        );
        let mut m = Cooperative::default();
        m.wake_cell((1, 2, 3));
        let saved = m.save();
        let mut r = Cooperative::default();
        r.load(&saved).unwrap();
        assert_eq!(r.save(), saved);
    }
}
