// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! A managed-package library for short, repeatable headless simulation demonstrations.
use pwc_module_api::block::{AIR, BlockId, BlockRegistry};
use pwc_module_api::material::{Configuration, Element, QUANTUM};
use pwc_module_api::sim::{
    model::ModelSpec,
    reactions::{Budget, CellStore, Pos, ReactionScheduler},
};
use pwc_module_api::{GameBuild, PackageInfo, PackageKind};
use std::collections::BTreeMap;

/// Separately selectable headless modes. `off` is a core without a provider.
pub const MODELS: &[&str] = &[
    "off",
    "selective",
    "wave",
    "packets",
    "reservoir",
    "rotor",
    "fuel",
    "cyclic",
    "loops",
    "spin",
    "symplectic",
    "bonds",
    "cooperative",
    "permutation",
    "dipole",
    "frames",
];
/// Resolve a named native factory without instantiating a second game runtime.
pub fn spec(name: &str) -> Result<Option<ModelSpec>, String> {
    Ok(match name {
        "off" => None,
        "selective" => Some(pwc_physics_selective::spec()),
        "wave" => Some(pwc_physics_wave::spec()),
        "packets" => Some(pwc_physics_packets::spec()),
        "reservoir" => Some(pwc_physics_reservoir::spec()),
        "rotor" => Some(pwc_physics_rotor::spec()),
        "fuel" => Some(pwc_physics_fuel::spec()),
        "cyclic" => Some(pwc_physics_cyclic::spec()),
        "loops" => Some(pwc_physics_loops::spec()),
        "spin" => Some(pwc_physics_spin::spec()),
        "symplectic" => Some(pwc_physics_symplectic::spec()),
        "bonds" => Some(pwc_physics_bonds::spec()),
        "cooperative" => Some(pwc_physics_cooperative::spec()),
        "permutation" => Some(pwc_physics_permutation::spec()),
        "dipole" => Some(pwc_physics_dipole::spec()),
        "frames" => Some(pwc_physics_frames::spec()),
        _ => return Err(format!("unknown model {name}")),
    })
}
/// A test build that registers the same canonical package entry points as the manager.
/// The actual generated runtime bundle carries the lock's package metadata and environment.
pub fn test_build() -> GameBuild {
    let mut build = GameBuild::new();
    build = build.with_package(PackageInfo {
        id: "pwc.physics-selective",
        name: "selective",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_selective::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-wave",
        name: "wave",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_wave::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-packets",
        name: "packets",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_packets::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-reservoir",
        name: "reservoir",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_reservoir::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-rotor",
        name: "rotor",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_rotor::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-fuel",
        name: "fuel",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_fuel::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-cyclic",
        name: "cyclic",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_cyclic::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-loops",
        name: "loops",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_loops::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-spin",
        name: "spin",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_spin::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-symplectic",
        name: "symplectic",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_symplectic::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-bonds",
        name: "bonds",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_bonds::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-cooperative",
        name: "cooperative",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_cooperative::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-permutation",
        name: "permutation",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_permutation::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-dipole",
        name: "dipole",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_dipole::register),
    });
    build = build.with_package(PackageInfo {
        id: "pwc.physics-frames",
        name: "frames",
        version: "0.1.0",
        description: "Independent native physics laboratory",
        kind: PackageKind::Module,
        dependencies: &[],
        register: Some(pwc_physics_frames::register),
    });
    build
}
/// One instrumentation sample. Operation and pending units are defined by each model.
#[derive(Debug, Clone)]
pub struct Readout {
    /// Stable selected model id, or `off`.
    pub model: &'static str,
    /// Number of requested host ticks.
    pub tick: usize,
    /// Whole phases accepted by the model.
    pub turns: u64,
    /// Model-specific committed/processed work count.
    pub operations: u64,
    /// Persistent active state or queued work count.
    pub pending: usize,
    /// Refused inputs/work reported by the model.
    pub dropped: u64,
    /// Ordinary material occurrences in the sparse matter fixture.
    pub matter: usize,
    /// Model-specific numeric instrument values, sampled at the origin.
    pub sample: [i64; 4],
    /// Key/value text bytes retained for the model; excludes binary save overhead.
    pub state_bytes: usize,
}
impl std::fmt::Display for Readout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{\"model\":\"{}\",\"tick\":{},\"turns\":{},\"operations\":{},\"pending\":{},\"dropped\":{},\"matter\":{},\"sample\":{:?},\"state_bytes\":{}}}",
            self.model,
            self.tick,
            self.turns,
            self.operations,
            self.pending,
            self.dropped,
            self.matter,
            self.sample,
            self.state_bytes
        )
    }
}
struct Map {
    cells: BTreeMap<Pos, BlockId>,
    registry: BlockRegistry,
}
impl CellStore for Map {
    fn block_at(&self, p: Pos) -> Option<BlockId> {
        Some(self.cells.get(&p).copied().unwrap_or(AIR))
    }
    fn set_block(&mut self, p: Pos, id: BlockId) -> Option<BlockId> {
        let old = self.block_at(p)?;
        if id == AIR {
            self.cells.remove(&p);
        } else {
            self.cells.insert(p, id);
        }
        Some(old)
    }
    fn registry(&self) -> &BlockRegistry {
        &self.registry
    }
    fn registry_mut(&mut self) -> &mut BlockRegistry {
        &mut self.registry
    }
    fn set_pair(
        &mut self,
        p: Pos,
        a: BlockId,
        na: BlockId,
        q: Pos,
        b: BlockId,
        nb: BlockId,
    ) -> bool {
        if p == q
            || self.block_at(p) != Some(a)
            || self.block_at(q) != Some(b)
            || usize::from(na.0) >= self.registry.block_count()
            || usize::from(nb.0) >= self.registry.block_count()
        {
            return false;
        }
        self.set_block(p, na);
        self.set_block(q, nb);
        true
    }
}
/// Run a deliberately small fixture, sampling every eight requested ticks and verifying the
/// complete model record round-trip. This is not a throughput or gameplay benchmark.
pub fn run(name: &str, ticks: usize) -> Result<Vec<Readout>, String> {
    let selected = spec(name)?;
    let mut host = ReactionScheduler::disabled();
    host.configure(selected)?;
    let mut map = Map {
        cells: BTreeMap::new(),
        registry: BlockRegistry::with_builtins(),
    };
    let cfg = |raw: [[u8; 4]; 4]| Configuration::new(raw.map(Element::new).to_vec()).unwrap();
    if matches!(name, "selective" | "reservoir") {
        let a = cfg([
            [73, 145, 162, 161],
            [71, 77, 157, 208],
            [34, 125, 217, 144],
            [8, 85, 210, 206],
        ]);
        let b = cfg([
            [83, 135, 211, 195],
            [51, 125, 144, 147],
            [11, 72, 167, 145],
            [25, 80, 211, 204],
        ]);
        let (a, b) = (
            map.registry.intern(&a).unwrap(),
            map.registry.intern(&b).unwrap(),
        );
        map.cells.insert((0, 0, 0), a);
        map.cells.insert((1, 0, 0), b);
        host.wake_cell((0, 0, 0));
        host.wake_cell((1, 0, 0));
        if name == "reservoir" {
            host.perturb((0, 0, 0), 64 * i64::from(QUANTUM));
        }
    } else if name == "cooperative" {
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
        let (a, b) = (
            map.registry.intern(&a).unwrap(),
            map.registry.intern(&b).unwrap(),
        );
        map.cells.insert((0, 0, 0), a);
        map.cells.insert((1, 0, 0), b);
        host.wake_cell((0, 0, 0));
        host.wake_cell((1, 0, 0));
    } else if name == "permutation" {
        let a = map
            .registry
            .intern(&Configuration::single(Element::new([1, 0, 0, 0])))
            .unwrap();
        map.cells.insert((0, 0, 0), a);
        host.perturb((0, 0, 0), 1);
    } else if name == "wave" {
        host.perturb((0, 0, 0), 4096);
        host.perturb((1, 0, 0), -4096);
    } else if name == "packets" {
        for d in [1, 3, 4] {
            host.perturb((0, 0, 0), d);
        }
    } else if name == "symplectic" {
        host.perturb((0, 0, 0), 4096);
        host.perturb((1, 0, 0), -4096);
    } else if name == "bonds" {
        host.perturb((0, 0, 0), 32);
    } else if name == "spin" {
        host.perturb((0, 0, 0), 64);
    } else if name != "off" {
        host.perturb((0, 0, 0), 1);
    }
    let mut rows = Vec::new();
    let mut changes = Vec::new();
    for tick in 0..=ticks {
        if tick % 8 == 0 || tick == ticks {
            let s = host.stats();
            rows.push(Readout {
                model: selected.map_or("off", |s| s.id),
                tick,
                turns: s.turns,
                operations: s.operations,
                pending: s.pending,
                dropped: s.dropped,
                matter: map
                    .cells
                    .values()
                    .map(|&id| map.registry.configuration(id).len())
                    .sum(),
                sample: host.sample((0, 0, 0)).values,
                state_bytes: host
                    .state_records()
                    .iter()
                    .map(|(k, v)| k.len() + v.len())
                    .sum(),
            });
        }
        if tick < ticks {
            changes.clear();
            host.tick_into(&mut map, Budget::DEFAULT, &mut changes);
        }
    }
    let records = host.state_records();
    let mut restored = ReactionScheduler::disabled();
    restored.load_records(&records)?;
    restored.configure(selected)?;
    if records != restored.state_records() {
        return Err(format!("{name} failed exact complete state round-trip"));
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    use pwc_module_api::{settings::OptionValue, testing::Harness};
    #[test]
    fn managed_registration_selects_exactly_the_requested_factory_and_pauses() {
        for &name in MODELS {
            let mut h = Harness::new(test_build());
            for &other in MODELS.iter().filter(|&&n| n != "off") {
                assert!(h.set_option(
                    &format!("pwc.physics-{other}.enabled"),
                    OptionValue::Bool(other == name)
                ));
            }
            assert_eq!(
                h.reaction_model().map(|s| s.id),
                spec(name).unwrap().map(|s| s.id)
            );
        }
    }
    #[test]
    fn short_all_model_demonstrations_and_full_state_round_trips() {
        let selected = std::env::var("PWC_PHYSICS_MODEL").ok();
        if let Some(names) = selected.as_deref() {
            for name in names.split(',').map(str::trim) {
                spec(name).expect("PWC_PHYSICS_MODEL must contain valid mode names");
            }
        }
        for &name in MODELS {
            if selected
                .as_deref()
                .is_some_and(|names| !names.split(',').any(|n| n.trim() == name))
            {
                continue;
            }
            for row in run(name, 32).unwrap() {
                println!("{row}");
            }
        }
    }
}
