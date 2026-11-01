// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sparse conservative channel dynamics. This is not a proved universal computer.
use pwc_mod_api::Action;
use pwc_mod_api::engine::Key;
use pwc_mod_api::input::intent::{Chord, Mods, Source};
use pwc_mod_api::material::{Configuration, fit_raw};
use pwc_mod_api::prelude::*;
use pwc_mod_api::sim::{model::*, reactions::*};
use std::collections::BTreeMap;

const ID: &str = "pwc.physics-packets";
const MAX_PACKETS: usize = 256;

pub fn register(registrar: &mut ModRegistrar) {
    let enabled = registrar.option(OptionSpec::toggle(
        "enabled",
        "Conservative Packet Physics",
        Category::World,
        false,
    ));
    registrar.add(PacketMod { enabled, on: false });
}

pub fn spec() -> ModelSpec {
    ModelSpec {
        id: ID,
        version: 1,
        create: || Box::new(Packets::default()),
    }
}

pub struct PacketMod {
    enabled: OptionId,
    on: bool,
}
impl Mod for PacketMod {
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
            id: "physics.packets.inject",
            label: "Packet laboratory injection",
            default: &[Chord::with(Mods::CTRL, Source::Key(Key::F2))],
            repeat: false,
            held: false,
            immediate: false,
        }];
        ACTIONS
    }
    fn update(&mut self, ctx: &mut ModContext) {
        if !ctx.networked
            && ctx.world.simulation_model_id() == Some(ID)
            && ctx.action("physics.packets.inject")
        {
            let pos = ctx.world.simulation_cell(ctx.player.position);
            ctx.world.perturb_simulation(pos, 1);
        }
    }
    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        if world.simulation_model_id() != Some(ID) || !facts.hud_mode.shows_mod_hud() {
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
                "Packets mask={:06b} count={} cells={} steps={} | Ctrl+F2 inject",
                s.values[0], s.values[1], s.values[2], stats.turns
            )
            .into(),
        });
    }
}

/// Bit order: +x,-x,+y,-y,+z,-z. All local scattering is a bijection on 64 states.
#[derive(Default)]
pub struct Packets {
    cells: BTreeMap<Pos, u8>,
    turns: u64,
    operations: u64,
    dropped: u64,
}

fn neighbor(p: Pos, channel: u8) -> Option<Pos> {
    let delta = if channel % 2 == 0 { 1 } else { -1 };
    Some(match channel / 2 {
        0 => (p.0.checked_add(delta)?, p.1, p.2),
        1 => (p.0, p.1.checked_add(delta)?, p.2),
        _ => (p.0, p.1, p.2.checked_add(delta)?),
    })
}

/// A head-on pair changes axis only in the presence of an odd spectator occupation.
/// The spectator bits never change, so applying this twice restores the original mask.
fn scatter(mask: u8, control_axis: u8) -> u8 {
    let c = 2 * control_axis;
    if ((mask >> c) & 3).count_ones() != 1 {
        return mask;
    }
    let a = 2 * ((control_axis + 1) % 3);
    let b = 2 * ((control_axis + 2) % 3);
    let (pa, pb) = ((mask >> a) & 3, (mask >> b) & 3);
    if !((pa == 3 && pb == 0) || (pa == 0 && pb == 3)) {
        return mask;
    }
    mask ^ ((3 << a) | (3 << b))
}

fn control_axis(cfg: &Configuration) -> u8 {
    (cfg.elements()
        .iter()
        .flat_map(|e| e.0)
        .map(u32::from)
        .sum::<u32>()
        % 3) as u8
}

/// Empty/occupied interfaces reflect. Two occupied configurations pass signals when their
/// summed cross-fit is nonnegative. Both voids pass. There are no named mirror/wire materials.
fn compatible(a: &Configuration, b: &Configuration) -> bool {
    if a.is_empty() || b.is_empty() {
        return a.is_empty() && b.is_empty();
    }
    a.elements()
        .iter()
        .flat_map(|&x| b.elements().iter().map(move |&y| i64::from(fit_raw(x, y))))
        .sum::<i64>()
        >= 0
}

impl Packets {
    fn count(&self) -> usize {
        self.cells.values().map(|m| m.count_ones() as usize).sum()
    }
}

impl Model for Packets {
    fn id(&self) -> &'static str {
        ID
    }
    fn wake_cell(&mut self, _pos: Pos) {}
    fn perturb(&mut self, pos: Pos, amount: i64) {
        if self.count() >= MAX_PACKETS || amount == 0 {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let mask = self.cells.get(&pos).copied().unwrap_or(0);
        let start = ((amount.rem_euclid(6) + 5) % 6) as u8;
        for delta in 0..6 {
            let bit = 1 << ((start + delta) % 6);
            if mask & bit == 0 {
                self.cells.insert(pos, mask | bit);
                return;
            }
        }
        self.dropped = self.dropped.saturating_add(1);
    }
    fn tick(&mut self, store: &mut dyn CellStore, budget: Budget, _out: &mut Vec<Mutation>) {
        if self.cells.is_empty() || self.cells.len() > budget.contacts_per_turn {
            return;
        }
        let mut next = BTreeMap::<Pos, u8>::new();
        for (&pos, &mask) in &self.cells {
            let Some(id) = store.block_at(store.resolve(pos)) else {
                self.dropped = self.dropped.saturating_add(1);
                return; // Keep every packet when the medium becomes unreadable.
            };
            let cfg = store.registry().configuration(id);
            let outgoing = scatter(mask, control_axis(cfg));
            for channel in 0..6 {
                if outgoing & (1 << channel) == 0 {
                    continue;
                }
                let n = neighbor(pos, channel);
                if n.is_some_and(|p| store.resolve(p) != p) || store.resolve(pos) != pos {
                    // A chart crossing needs a channel-frame transform, which this lab lacks.
                    self.dropped = self.dropped.saturating_add(1);
                    return;
                }
                let open = n
                    .and_then(|p| store.block_at(store.resolve(p)))
                    .is_some_and(|nid| compatible(cfg, store.registry().configuration(nid)));
                let (dest, out_channel) = if open {
                    (store.resolve(n.unwrap()), channel)
                } else {
                    (pos, channel ^ 1)
                };
                let dest_mask = next.entry(dest).or_default();
                if *dest_mask & (1 << out_channel) != 0 {
                    // Chart direction rotation is not supported; never silently merge packets.
                    self.dropped = self.dropped.saturating_add(1);
                    return;
                }
                *dest_mask |= 1 << out_channel;
            }
        }
        debug_assert_eq!(
            self.count(),
            next.values().map(|m| m.count_ones() as usize).sum()
        );
        self.operations = self.operations.saturating_add(self.cells.len() as u64);
        self.cells = next;
        self.turns = self.turns.saturating_add(1);
    }
    fn sample(&self, pos: Pos) -> Sample {
        let mask = self.cells.get(&pos).copied().unwrap_or(0);
        Sample {
            labels: [
                "direction mask",
                "total packets",
                "occupied cells",
                "local packets",
            ],
            values: [
                i64::from(mask),
                self.count() as i64,
                self.cells.len() as i64,
                i64::from(mask.count_ones()),
            ],
        }
    }
    fn stats(&self) -> Stats {
        Stats {
            turns: self.turns,
            operations: self.operations,
            dropped: self.dropped,
            pending: self.cells.len(),
        }
    }
    fn save(&self) -> String {
        let mut s = format!(
            "packets1 {} {} {}\n",
            self.turns, self.operations, self.dropped
        );
        for (&(x, y, z), &mask) in &self.cells {
            s.push_str(&format!("{x} {y} {z} {mask}\n"));
        }
        s
    }
    fn load(&mut self, data: &str) -> Result<(), String> {
        let mut lines = data.lines();
        let h: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
        if h.len() != 4 || h[0] != "packets1" {
            return Err("unsupported packet state".into());
        }
        let counter = |s: &str| {
            s.parse::<u64>()
                .map_err(|_| "bad packet counter".to_owned())
        };
        let mut next = Packets {
            turns: counter(h[1])?,
            operations: counter(h[2])?,
            dropped: counter(h[3])?,
            ..Self::default()
        };
        let mut count = 0usize;
        for line in lines {
            if next.cells.len() >= MAX_PACKETS {
                return Err("too many packet cells".into());
            }
            let v: Vec<_> = line.split_whitespace().collect();
            if v.len() != 4 {
                return Err("bad packet row".into());
            }
            let coord = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "bad packet position".to_owned())
            };
            let mask = v[3]
                .parse::<u8>()
                .map_err(|_| "bad direction mask".to_owned())?;
            if mask == 0 || mask > 63 {
                return Err("invalid direction mask".into());
            }
            count += mask.count_ones() as usize;
            if count > MAX_PACKETS {
                return Err("packet count exceeds capacity".into());
            }
            if next
                .cells
                .insert((coord(v[0])?, coord(v[1])?, coord(v[2])?), mask)
                .is_some()
            {
                return Err("duplicate packet position".into());
            }
        }
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_local_state_is_reversible_and_conserves_number_and_momentum() {
        let momentum = |mask: u8| {
            std::array::from_fn::<_, 3, _>(|axis| {
                ((mask >> (2 * axis)) & 1) as i8 - ((mask >> (2 * axis + 1)) & 1) as i8
            })
        };
        for axis in 0..3 {
            for mask in 0u8..64 {
                let next = scatter(mask, axis);
                assert_eq!(scatter(next, axis), mask);
                assert_eq!(next.count_ones(), mask.count_ones());
                assert_eq!(momentum(next), momentum(mask));
            }
        }
        assert_eq!(
            scatter(0b01_00_11, 2),
            0b01_11_00,
            "a spectator routes a head-on pair"
        );
        assert_eq!(
            scatter(0b00_00_11, 2),
            0b00_00_11,
            "the same pair passes without a spectator"
        );
        println!(
            "all 192 axis/state cases conserve packet count and local momentum; scattering is involutive"
        );
    }
}
