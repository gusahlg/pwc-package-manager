// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The game-facing body: one headless player, translated into [`crate::body::Body`].
//!
//! Block distances become metres (`BLOCK_METERS` = 0.85). A material's key is the FNV-1a 64 of
//! its canonical spec, except `"air"`, which is [`crate::facts::AIR`]. Rays and the target use
//! the client's own raycast, which follows a round world's patches, and distances embed its
//! storage cells in world space. `placeable` is "the cell in front is empty"; overlap with the
//! body is left to the client's `Refusal::Occupied`.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use pwc_mod_api::block::{self, BlockId};
use pwc_mod_api::engine::DVec3;
use pwc_mod_api::headless::{self, HeadlessClient, JoinOptions};
use pwc_mod_api::math::BLOCK_METERS;
use pwc_mod_api::player::STANDARD_GRAVITY;
use pwc_mod_api::{CliContext, PackageKind};

use crate::body::{
    Act, Body, BodyEvent, Connect, Controls, PeerView, RayBasis, RayDir, RayHit, Snapshot, Target,
};
use crate::cli::{self, Invocation};
use crate::facts::{Cell, EditKind, MATERIAL_FEATURES, Material, Outcome, Pose, Refusal};
use crate::swarm::ServerArgs;

const JOIN_TIMEOUT: Duration = Duration::from_secs(20);
/// Same cut the game uses before it will name an up (`0.02 * STANDARD_GRAVITY`, in blocks/s²).
const UP_MIN: f64 = 0.02;

/// Join options for every agent of one run. The mod list is the build's mod packages.
pub struct ClientConnect {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub mods: Vec<(String, String)>,
}

impl Connect for ClientConnect {
    fn connect(&self, name: &str) -> Result<Box<dyn Body>, String> {
        let opts = JoinOptions {
            host: self.host.clone(),
            port: self.port,
            name: name.to_string(),
            password: self.password.clone(),
            mods: self.mods.clone(),
            timeout: JOIN_TIMEOUT,
        };
        match HeadlessClient::connect(&opts) {
            Ok(client) => Ok(Box::new(GameBody::new(client))),
            Err(err) => {
                let mut message = err.to_string();
                if !err.mods_denied.is_empty() {
                    message.push_str(" (mods denied: ");
                    message.push_str(&err.mods_denied.join(", "));
                    message.push(')');
                }
                Err(message)
            }
        }
    }
}

/// `swarm` from the game's command line. Mods sent at join are every `PackageKind::Mod` in the
/// build; the CLI context does not say which of them are suspended.
pub fn entry(ctx: &CliContext) -> i32 {
    let mods: Vec<(String, String)> = ctx
        .build
        .packages()
        .iter()
        .filter(|package| package.kind == PackageKind::Mod)
        .map(|package| (package.id.to_string(), package.version.to_string()))
        .collect();
    let invocation = Invocation {
        args: ctx.args.clone(),
        data_dir: ctx.data_dir.clone(),
        mods: mods.clone(),
    };
    cli::run(invocation, &|server: &ServerArgs| {
        Box::new(ClientConnect {
            host: server.host.clone(),
            port: server.port,
            password: server.password.clone(),
            mods: mods.clone(),
        })
    })
}

struct PendingEdit {
    kind: EditKind,
    before: Material,
    after: Material,
}

struct GameBody {
    client: HeadlessClient,
    cache: Vec<Option<Material>>,
    ids: BTreeMap<u64, BlockId>,
    held: BTreeMap<u64, (Material, u32)>,
    mirror: BTreeMap<Cell, Material>,
    pending_edit: HashMap<Cell, PendingEdit>,
    pending_tool: HashMap<Cell, (Material, Material)>,
    alive: bool,
}

impl GameBody {
    fn new(client: HeadlessClient) -> Self {
        let alive = client.alive();
        let mut body = Self {
            client,
            cache: Vec::new(),
            ids: BTreeMap::new(),
            held: BTreeMap::new(),
            mirror: BTreeMap::new(),
            pending_edit: HashMap::new(),
            pending_tool: HashMap::new(),
            alive,
        };
        body.sync_inventory(&mut Vec::new(), true);
        body
    }

    fn material_of(&mut self, id: BlockId) -> Material {
        let index = id.0 as usize;
        if let Some(Some(material)) = self.cache.get(index) {
            return *material;
        }
        let material = observe(&self.client, id);
        if self.cache.len() <= index {
            self.cache.resize(index + 1, None);
        }
        self.cache[index] = Some(material);
        if !material.is_air() {
            self.ids.insert(material.key, id);
        }
        material
    }

    /// Metres from the eye to a cell's centre, in world space (round-world cells are embedded).
    fn cell_distance(&self, eye: DVec3, cell: Cell) -> f32 {
        ((self.client.cell_center(cell) - eye).length() * BLOCK_METERS) as f32
    }

    fn remember(&mut self, cell: Cell, material: Material) {
        self.mirror.insert(cell, material);
    }

    fn sync_inventory(&mut self, events: &mut Vec<BodyEvent>, silent: bool) {
        let mut now: BTreeMap<u64, (Material, u32)> = BTreeMap::new();
        let pairs: Vec<(BlockId, u32)> = self.client.player().inventory.iter().collect();
        for (id, count) in pairs {
            if count == 0 {
                continue;
            }
            let material = self.material_of(id);
            if material.is_air() {
                continue;
            }
            let slot = now.entry(material.key).or_insert((material, 0));
            slot.1 = slot.1.saturating_add(count);
        }
        if !silent {
            let mut keys: Vec<u64> = self
                .held
                .keys()
                .copied()
                .chain(now.keys().copied())
                .collect();
            keys.sort_unstable();
            keys.dedup();
            for key in keys {
                let before = self.held.get(&key).map(|(_, count)| *count).unwrap_or(0);
                let (material, after) = now.get(&key).copied().unwrap_or((
                    self.held
                        .get(&key)
                        .map(|(material, _)| *material)
                        .unwrap_or(Material::AIR),
                    0,
                ));
                let delta = after as i32 - before as i32;
                if delta != 0 {
                    events.push(BodyEvent::Inventory { material, delta });
                }
            }
        }
        self.held = now;
    }

    fn to_input(&mut self, controls: &Controls, events: &mut Vec<BodyEvent>) -> headless::Input {
        let act = match controls.act {
            None => None,
            Some(Act::Break) => Some(headless::Act::Break),
            Some(Act::Swing) => Some(headless::Act::Swing),
            Some(Act::Place(key)) => match self.ids.get(&key).copied() {
                Some(id) => Some(headless::Act::Place(id)),
                None => {
                    events.push(BodyEvent::Refused(Refusal::NotHeld));
                    None
                }
            },
            Some(Act::UseTool(key)) => match self.ids.get(&key).copied() {
                Some(id) => {
                    if let Some(target) = self.client.target() {
                        let cell_before = self.material_of(target.block);
                        let tool_before = self.material_of(id);
                        self.pending_tool
                            .insert(target.cell, (cell_before, tool_before));
                        self.remember(target.cell, cell_before);
                    }
                    Some(headless::Act::UseTool(id))
                }
                None => {
                    events.push(BodyEvent::Refused(Refusal::NotHeld));
                    None
                }
            },
        };
        headless::Input {
            forward: controls.forward,
            right: controls.right,
            jump: controls.jump,
            sprint: controls.sprint,
            sneak: controls.sneak,
            turn: controls.turn,
            tilt: controls.tilt,
            act,
        }
    }
}

impl Body for GameBody {
    fn sense(&mut self, rays: &[RayDir], range: f32, radius: f32, out: &mut Snapshot) {
        out.rays.clear();
        out.peers.clear();
        out.inventory.clear();
        let eye = self.client.eye();
        let (fwd_h, right) = self.client.player().movement_basis();
        let up = self.client.up();
        let (axis_x, axis_z) = frame_axes(self.client.yaw(), fwd_h, right);
        let velocity = self.client.player().velocity() * BLOCK_METERS;
        out.body_velocity = [
            dot3(velocity, right),
            dot3(velocity, up),
            dot3(velocity, fwd_h),
        ];
        let gravity = self.client.world().gravity_at(eye);
        out.gravity = (gravity.accel.length() * BLOCK_METERS) as f32;
        let up_dir = gravity.up(UP_MIN * STANDARD_GRAVITY);
        out.pose = Some(Pose {
            eye: [
                eye.x * BLOCK_METERS,
                eye.y * BLOCK_METERS,
                eye.z * BLOCK_METERS,
            ],
            velocity: [velocity.x as f32, velocity.y as f32, velocity.z as f32],
            up: up_dir.map(|dir| [dir.x, dir.y, dir.z]),
            grounded: self.client.on_ground(),
            in_air: self.client.world().in_air(eye),
        });
        out.yaw = self.client.yaw();
        out.pitch = self.client.pitch();
        out.time_of_day = self.client.time_of_day();
        let range_blocks = range as f64 / BLOCK_METERS;
        for dir in rays {
            let aim = aim(
                &axis_x,
                &up,
                &axis_z,
                self.client.yaw(),
                self.client.pitch(),
                dir,
            );
            let hit = match self.client.ray(aim, range_blocks) {
                Some(hit) => {
                    let material = self.material_of(hit.block);
                    self.remember(hit.cell, material);
                    RayHit {
                        distance: self.cell_distance(eye, hit.cell).min(range),
                        material,
                    }
                }
                None => RayHit {
                    distance: range,
                    material: Material::AIR,
                },
            };
            out.rays.push(hit);
        }
        // The client's own raycast (atlas-aware on round worlds), so a legal act is one it accepts.
        out.target = self.client.target().map(|hit| {
            let material = self.material_of(hit.block);
            self.remember(hit.cell, material);
            let (bx, by, bz) = hit.before;
            Target {
                cell: hit.cell,
                distance: self.cell_distance(eye, hit.cell),
                material,
                placeable: hit.before != hit.cell && !self.client.world().is_solid(bx, by, bz),
            }
        });
        let pairs: Vec<(BlockId, u32)> = self.client.player().inventory.iter().collect();
        for (id, count) in pairs {
            if count == 0 {
                continue;
            }
            let material = self.material_of(id);
            if material.is_air() {
                continue;
            }
            if let Some(slot) = out
                .inventory
                .iter_mut()
                .find(|(have, _)| have.key == material.key)
            {
                slot.1 = slot.1.saturating_add(count);
            } else {
                out.inventory.push((material, count));
            }
        }
        out.inventory_total = self.client.player().inventory.total();
        out.inventory_capacity = self.client.player().inventory.capacity() as u32;
        let mut peers = Vec::new();
        for peer in self.client.peers() {
            let delta = (peer.eye - eye) * BLOCK_METERS;
            let distance = delta.length() as f32;
            if !distance.is_finite() || distance > radius {
                continue;
            }
            peers.push(PeerView {
                offset: [dot3(delta, right), dot3(delta, up), dot3(delta, fwd_h)],
                distance,
            });
        }
        peers.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        out.peers = peers;
    }

    fn step(&mut self, controls: &Controls, dt: f32, radius: f32, events: &mut Vec<BodyEvent>) {
        let input = self.to_input(controls, events);
        let stepped = self.client.step(&input, dt);
        self.translate(stepped.events, radius, events);
        self.sync_inventory(events, false);
        self.alive = self.client.alive();
    }

    fn alive(&self) -> bool {
        self.alive
    }
}

impl GameBody {
    fn translate(
        &mut self,
        incoming: Vec<headless::Event>,
        radius: f32,
        events: &mut Vec<BodyEvent>,
    ) {
        let eye = self.client.eye();
        let mut own = Vec::new();
        let mut changed = Vec::new();
        for event in incoming {
            match event {
                headless::Event::Broke { cell, block } => {
                    let before = self.material_of(block);
                    let after = Material::AIR;
                    self.remember(cell, after);
                    self.pending_edit.insert(
                        cell,
                        PendingEdit {
                            kind: EditKind::Broke,
                            before,
                            after,
                        },
                    );
                    own.push(cell);
                    events.push(BodyEvent::Edit {
                        cell,
                        kind: EditKind::Broke,
                        before,
                        after,
                        outcome: Outcome::Predicted,
                    });
                }
                headless::Event::Placed { cell, block } => {
                    let after = self.material_of(block);
                    let before = self.mirror.get(&cell).copied().unwrap_or(Material::AIR);
                    self.remember(cell, after);
                    self.pending_edit.insert(
                        cell,
                        PendingEdit {
                            kind: EditKind::Placed,
                            before,
                            after,
                        },
                    );
                    own.push(cell);
                    events.push(BodyEvent::Edit {
                        cell,
                        kind: EditKind::Placed,
                        before,
                        after,
                        outcome: Outcome::Predicted,
                    });
                }
                headless::Event::EditConfirmed { cell } => {
                    if let Some(pending) = self.pending_edit.remove(&cell) {
                        self.remember(cell, pending.after);
                        events.push(BodyEvent::Edit {
                            cell,
                            kind: pending.kind,
                            before: pending.before,
                            after: pending.after,
                            outcome: Outcome::Confirmed,
                        });
                    }
                }
                headless::Event::EditRejected { cell } => {
                    if let Some(pending) = self.pending_edit.remove(&cell) {
                        self.remember(cell, pending.before);
                        events.push(BodyEvent::Edit {
                            cell,
                            kind: pending.kind,
                            before: pending.before,
                            after: pending.after,
                            outcome: Outcome::Rejected,
                        });
                    }
                }
                headless::Event::ToolResult {
                    cell,
                    reacted,
                    cell_block,
                    tool_block,
                } => {
                    let (cell_before, tool_before) = self
                        .pending_tool
                        .remove(&cell)
                        .unwrap_or((Material::AIR, Material::AIR));
                    let cell_after = self.material_of(cell_block);
                    let tool_after = self.material_of(tool_block);
                    self.remember(cell, cell_after);
                    own.push(cell);
                    events.push(BodyEvent::Tool {
                        cell,
                        cell_before,
                        tool_before,
                        cell_after,
                        tool_after,
                        reacted,
                    });
                }
                headless::Event::Refused(reason) => {
                    events.push(BodyEvent::Refused(map_refusal(reason)))
                }
                headless::Event::SnappedBack { .. } => events.push(BodyEvent::SnappedBack),
                headless::Event::WorldChanged { cell, block } => {
                    let after = self.material_of(block);
                    changed.push((cell, after));
                }
                headless::Event::Disconnected { reason } => {
                    self.alive = false;
                    events.push(BodyEvent::Disconnected { reason });
                }
                headless::Event::Chat { .. }
                | headless::Event::Joined { .. }
                | headless::Event::Left { .. } => {}
            }
        }
        for (cell, after) in changed {
            if own.contains(&cell) {
                self.remember(cell, after);
                continue;
            }
            let before = self.mirror.get(&cell).copied().unwrap_or(Material::AIR);
            self.remember(cell, after);
            if before.key == after.key {
                continue;
            }
            let distance = self.cell_distance(eye, cell);
            if distance <= radius {
                events.push(BodyEvent::Mutation {
                    cell,
                    before,
                    after,
                    distance,
                });
            }
        }
    }
}

fn map_refusal(reason: headless::Refusal) -> Refusal {
    match reason {
        headless::Refusal::NoTarget => Refusal::NoTarget,
        headless::Refusal::Cooldown => Refusal::Cooldown,
        headless::Refusal::NotHeld => Refusal::NotHeld,
        headless::Refusal::Occupied => Refusal::Occupied,
    }
}

fn observe(client: &HeadlessClient, id: BlockId) -> Material {
    if id == block::AIR {
        return Material::AIR;
    }
    let registry = client.world().registry();
    let spec = registry.spec_ref(id);
    if spec.as_ref() == "air" {
        return Material::AIR;
    }
    let observation = registry.observation(id);
    let amount = registry.amount(id);
    let cohesion = ((observation.cohesion as f32) + 32768.0) / 65535.0;
    Material {
        key: fnv1a(spec.as_ref()),
        features: [
            if observation.solid { 1.0 } else { 0.0 },
            observation.hardness as f32 / 255.0,
            observation.friction as f32 / 255.0,
            observation.transparency as f32 / 255.0,
            observation.emission as f32 / 15.0,
            cohesion.clamp(0.0, 1.0),
            (amount as f32 / 32.0).clamp(0.0, 1.0),
        ],
    }
}

fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn dot3(a: DVec3, b: DVec3) -> f32 {
    a.dot(b) as f32
}

/// Local +X and +Z of the body frame, recovered from the horizontal basis at `yaw`.
fn frame_axes(yaw: f32, forward: DVec3, right: DVec3) -> (DVec3, DVec3) {
    let (sin_yaw, cos_yaw) = (yaw as f64).sin_cos();
    let axis_x = forward * cos_yaw - right * sin_yaw;
    let axis_z = forward * sin_yaw + right * cos_yaw;
    (axis_x, axis_z)
}

fn aim(axis_x: &DVec3, up: &DVec3, axis_z: &DVec3, yaw: f32, pitch: f32, dir: &RayDir) -> DVec3 {
    match dir.basis {
        RayBasis::Up => *up,
        RayBasis::Down => -*up,
        RayBasis::View => {
            let (sin_yaw, cos_yaw) = ((yaw + dir.yaw) as f64).sin_cos();
            let (sin_pitch, cos_pitch) = ((pitch + dir.pitch) as f64).sin_cos();
            *axis_x * (cos_yaw * cos_pitch) + *up * sin_pitch + *axis_z * (sin_yaw * cos_pitch)
        }
    }
}
