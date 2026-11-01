// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Fixed observation layout. Values are scaled by hand into roughly `[-1, 1]`; nothing is tracked
//! online. `NORM_VERSION` rides in the archive header so a change of scale refuses old runs.
//!
//! Ray fan: 5 yaw offsets (±40°) by 3 pitch offsets (±25°) across the view, then straight down and
//! straight up along the body. 17 rays, range [`RAY_RANGE`] metres.
//!
//! Peer inputs (4): `count / 8`, the nearest peer's body-frame **right** and **up** (a unit
//! vector), and `distance / radius` in place of the forward component. All four are zero when
//! nobody is inside the sensing radius. The forward component is the one replaced, so a peer
//! directly ahead still reports how far they are.

use crate::actions::{self, HAND_SLOTS};
use crate::body::{RayBasis, RayDir, RayHit, Snapshot};
use crate::facts::{AIR, MATERIAL_FEATURES, Material};

pub const NORM_VERSION: u32 = 1;
pub const MEMORY: usize = 4;
pub const DESC_SLOTS: usize = 4;
pub const RAY_COUNT: usize = 17;
pub const RAY_FEATS: usize = 5;
pub const RAY_RANGE: f32 = 8.0;
/// Peers and mutations are reported inside this radius.
pub const SENSE_RADIUS: f32 = 32.0;
/// Target distance is divided by this (client reach is 6 m).
pub const TARGET_RANGE: f32 = 6.0;

const BODY: usize = 10;
const TARGET: usize = 1 + 1 + MATERIAL_FEATURES + 1;
const SLOT: usize = 1 + MATERIAL_FEATURES;
const HAND: usize = HAND_SLOTS * SLOT + 1;
const PEERS: usize = 4;
const TIME: usize = 2;

/// Yaw offsets across the view, degrees, left to right.
const YAWS_DEG: [f32; 5] = [-40.0, -20.0, 0.0, 20.0, 40.0];
/// Pitch offsets, degrees, down to up.
const PITCHES_DEG: [f32; 3] = [-25.0, 0.0, 25.0];

pub fn obs_len() -> usize {
    BODY + RAY_COUNT * RAY_FEATS
        + TARGET
        + HAND
        + PEERS
        + TIME
        + actions::ACTION_COUNT
        + MEMORY
        + DESC_SLOTS
}

/// The 17 rays in observation order.
pub fn ray_table() -> [RayDir; RAY_COUNT] {
    let mut rays = [RayDir::down(); RAY_COUNT];
    let mut n = 0;
    for yaw in YAWS_DEG {
        for pitch in PITCHES_DEG {
            rays[n] = RayDir::view(yaw.to_radians(), pitch.to_radians());
            n += 1;
        }
    }
    rays[n] = RayDir {
        yaw: 0.0,
        pitch: 0.0,
        basis: RayBasis::Down,
    };
    rays[n + 1] = RayDir::up();
    rays
}

/// Fill `out` (length [`obs_len`]) from `snap`. No allocation.
///
/// `slots` is the episode hand table: material and count per slot, empty slots as air with count 0.
/// `prev_action == u16::MAX` means "no previous action" (the one-hot stays zero). `memory` is the
/// network's previous memory output, already tanh-squashed; missing entries stay zero.
pub fn fill(
    snap: &Snapshot,
    slots: &[(Material, u32); HAND_SLOTS],
    prev_action: u16,
    memory: &[f32],
    out: &mut [f32],
) {
    let n = obs_len();
    if out.len() < n {
        return;
    }
    out[..n].fill(0.0);
    let mut i = 0;

    let vel = snap.body_velocity;
    let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
    let (grounded, in_air) = match snap.pose {
        Some(p) => (p.grounded, p.in_air),
        None => (false, false),
    };
    push(out, &mut i, finite(vel[0] / 10.0));
    push(out, &mut i, finite(vel[1] / 10.0));
    push(out, &mut i, finite(vel[2] / 10.0));
    push(out, &mut i, if grounded { 1.0 } else { 0.0 });
    push(out, &mut i, if in_air { 1.0 } else { 0.0 });
    push(out, &mut i, finite(speed / 10.0));
    push(
        out,
        &mut i,
        finite(snap.pitch / (std::f32::consts::FRAC_PI_2)),
    );
    push(out, &mut i, finite(snap.yaw.sin()));
    push(out, &mut i, finite(snap.yaw.cos()));
    push(out, &mut i, finite(snap.gravity / 30.0));

    for r in 0..RAY_COUNT {
        let hit = snap.rays.get(r).copied().unwrap_or(RayHit {
            distance: RAY_RANGE,
            material: Material::AIR,
        });
        let dist = if RAY_RANGE > 0.0 {
            hit.distance / RAY_RANGE
        } else {
            0.0
        };
        push(out, &mut i, finite(dist));
        push(out, &mut i, feat(&hit.material, 0));
        push(out, &mut i, feat(&hit.material, 1));
        push(out, &mut i, feat(&hit.material, 3));
        push(out, &mut i, feat(&hit.material, 4));
    }

    if let Some(t) = snap.target {
        push(out, &mut i, 1.0);
        push(out, &mut i, finite(t.distance / TARGET_RANGE));
        for f in 0..MATERIAL_FEATURES {
            push(out, &mut i, feat(&t.material, f));
        }
        push(out, &mut i, if t.placeable { 1.0 } else { 0.0 });
    } else {
        i += TARGET;
    }

    for slot in slots {
        let (mat, count) = if slot.0.key == AIR {
            (Material::AIR, 0)
        } else {
            *slot
        };
        let mag = if count == 0 {
            0.0
        } else {
            ((1.0 + count as f32).log2()) / 7.0
        };
        push(out, &mut i, finite(mag));
        for f in 0..MATERIAL_FEATURES {
            push(out, &mut i, if count == 0 { 0.0 } else { feat(&mat, f) });
        }
    }
    let fill_ratio = if snap.inventory_capacity == 0 {
        0.0
    } else {
        snap.inventory_total as f32 / snap.inventory_capacity as f32
    };
    push(out, &mut i, finite(fill_ratio));

    let near = snap.peers.len().min(255) as f32 / 8.0;
    push(out, &mut i, finite(near));
    if let Some(peer) = snap.peers.first() {
        let dist = peer.distance.max(1e-4);
        push(out, &mut i, finite(peer.offset[0] / dist));
        push(out, &mut i, finite(peer.offset[1] / dist));
        let radius = SENSE_RADIUS.max(1e-3);
        push(out, &mut i, finite(peer.distance / radius));
    } else {
        i += 3;
    }

    let angle = std::f32::consts::TAU * snap.time_of_day.clamp(0.0, 1.0);
    push(out, &mut i, finite(angle.sin()));
    push(out, &mut i, finite(angle.cos()));

    let prev = prev_action as usize;
    if prev < actions::ACTION_COUNT {
        out[i + prev] = 1.0;
    }
    i += actions::ACTION_COUNT;

    for m in 0..MEMORY {
        push(out, &mut i, finite(memory.get(m).copied().unwrap_or(0.0)));
    }
    // Descriptor slots stay zero: reserved for a later descriptor-conditioned actor.
    i += DESC_SLOTS;
    debug_assert_eq!(i, n);
}

fn push(out: &mut [f32], i: &mut usize, v: f32) {
    out[*i] = v;
    *i += 1;
}

fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

fn feat(mat: &Material, index: usize) -> f32 {
    finite(mat.features.get(index).copied().unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{PeerView, Target};
    use crate::facts::Pose;

    fn slot(key: u64, count: u32, solid: f32) -> (Material, u32) {
        let mut features = [0.0; MATERIAL_FEATURES];
        features[0] = solid;
        (Material { key, features }, count)
    }

    #[test]
    fn obs_len_is_the_documented_layout() {
        // body 10 + rays 17*5 + target 10 + hand 4*8+1 + peers 4 + time 2 + actions 22 + memory 4 + desc 4
        assert_eq!(obs_len(), 10 + 85 + 10 + 33 + 4 + 2 + 22 + 4 + 4);
        assert_eq!(obs_len(), 174);
        assert_eq!(ray_table().len(), RAY_COUNT);
        assert_eq!(ray_table()[15].basis, RayBasis::Down);
        assert_eq!(ray_table()[16].basis, RayBasis::Up);
        assert_eq!(ray_table()[0].basis, RayBasis::View);
    }

    #[test]
    fn values_are_finite_and_a_calm_snapshot_stays_near_unit_scale() {
        let mat = Material {
            key: 3,
            features: [1.0, 0.5, 0.2, 0.0, 0.1, 0.4, 0.2],
        };
        let mut snap = Snapshot {
            pose: Some(Pose {
                eye: [0.0, 1.6, 0.0],
                velocity: [0.0; 3],
                up: Some([0.0, 1.0, 0.0]),
                grounded: true,
                in_air: true,
            }),
            body_velocity: [2.0, 0.0, 4.0],
            yaw: 0.4,
            pitch: 0.2,
            gravity: 24.0,
            time_of_day: 0.25,
            inventory_total: 4,
            inventory_capacity: 16,
            rays: ray_table()
                .iter()
                .map(|_| RayHit {
                    distance: 4.0,
                    material: mat,
                })
                .collect(),
            target: Some(Target {
                cell: (1, 1, 1),
                distance: 3.0,
                material: mat,
                placeable: true,
            }),
            peers: vec![PeerView {
                offset: [0.0, 0.0, 10.0],
                distance: 10.0,
            }],
            ..Snapshot::default()
        };
        let slots = [
            slot(2, 4, 1.0),
            slot(AIR, 0, 0.0),
            slot(AIR, 0, 0.0),
            slot(AIR, 0, 0.0),
        ];
        let mut out = vec![0.0; obs_len()];
        let memory = [0.2, -0.3, 0.0, 0.5];
        fill(&snap, &slots, 3, &memory, &mut out);
        for (i, v) in out.iter().enumerate() {
            assert!(v.is_finite(), "obs[{i}] = {v}");
            assert!(v.abs() <= 2.0, "obs[{i}] = {v} left the rough unit range");
        }
        assert_eq!(
            out[obs_len() - DESC_SLOTS - MEMORY - actions::ACTION_COUNT + 3],
            1.0
        );
        assert_eq!(out[obs_len() - 1], 0.0, "descriptor slots stay zero");

        snap.body_velocity = [1.0e6, -1.0e6, 1.0e6];
        snap.gravity = 1.0e6;
        snap.pitch = 1.0e6;
        snap.rays.clear();
        snap.target = None;
        snap.peers.clear();
        let slots = [slot(9, u32::MAX, 1.0e6); 4];
        fill(&snap, &slots, u16::MAX, &memory, &mut out);
        for v in &out {
            assert!(v.is_finite());
        }
    }
}
