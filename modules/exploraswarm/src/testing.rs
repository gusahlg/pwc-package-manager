// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! A tiny voxel world for tests: flat ground, a ring of ore, one toy tool reaction.
//!
//! Distances are metres (`CELL_METRES` per cell). Yaw 0 looks along +Z. Physics can run
//! unpaced; the swarm's `paced` flag skips the sleep, this world does not keep a clock of its own.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::body::{
    Act, Body, BodyEvent, CELL_METRES, Connect, Controls, PeerView, RayBasis, RayDir, RayHit,
    Snapshot, Target,
};
use crate::facts::{Cell, EditKind, MATERIAL_FEATURES, Material, Outcome, Pose, Refusal};

const STONE: u64 = 1;
const DIRT: u64 = 2;
const ORE: u64 = 3;
const GEM: u64 = 4;
const PICK: u64 = 5;
const EYE: f64 = 1.62;
const WALK: f32 = 4.0;
const SPRINT: f32 = 7.0;
const SNEAK: f32 = 2.0;
const GRAVITY: f32 = 24.0;
const JUMP: f32 = 6.0;

fn material(key: u64) -> Material {
    let mut features = [0.0; MATERIAL_FEATURES];
    if key != crate::facts::AIR {
        features[0] = 1.0;
        features[1] = if key == STONE { 0.9 } else { 0.4 };
        features[2] = 0.5;
        features[6] = 0.1;
    }
    Material { key, features }
}

struct Change {
    source: u64,
    cell: Cell,
    before: Material,
    after: Material,
}

struct Shared {
    cells: BTreeMap<Cell, u64>,
    changes: Vec<Change>,
    poses: BTreeMap<u64, [f64; 3]>,
}

impl Shared {
    fn new() -> Self {
        let mut cells = BTreeMap::new();
        for x in -4i32..=4 {
            for z in -4i32..=4 {
                let ring = x.abs().max(z.abs());
                if (3..=4).contains(&ring) {
                    for y in 1..=3 {
                        cells.insert((x, y, z), ORE);
                    }
                }
            }
        }
        Self {
            cells,
            changes: Vec::new(),
            poses: BTreeMap::new(),
        }
    }

    fn get(&self, cell: Cell) -> u64 {
        if cell.1 <= 0 {
            STONE
        } else {
            self.cells.get(&cell).copied().unwrap_or(crate::facts::AIR)
        }
    }

    fn solid(&self, cell: Cell) -> bool {
        self.get(cell) != crate::facts::AIR
    }
}

/// Joins the shared fake world. `connects` counts successful joins, including reconnects.
#[derive(Clone)]
pub struct FakeConnect {
    world: Arc<Mutex<Shared>>,
    connects: Arc<AtomicU64>,
    next_id: Arc<AtomicU64>,
}

impl FakeConnect {
    pub fn new() -> Self {
        Self {
            world: Arc::new(Mutex::new(Shared::new())),
            connects: Arc::new(AtomicU64::new(0)),
            next_id: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn connects(&self) -> u64 {
        self.connects.load(Ordering::Relaxed)
    }
}

impl Default for FakeConnect {
    fn default() -> Self {
        Self::new()
    }
}

impl Connect for FakeConnect {
    fn connect(&self, _name: &str) -> Result<Box<dyn Body>, String> {
        self.connects.fetch_add(1, Ordering::Relaxed);
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FakeBody::spawn(id, Arc::clone(&self.world))))
    }
}

struct FakeBody {
    id: u64,
    world: Arc<Mutex<Shared>>,
    eye: [f64; 3],
    velocity: [f32; 3],
    yaw: f32,
    pitch: f32,
    grounded: bool,
    inventory: Vec<(Material, u32)>,
    total: u32,
    seen: usize,
    time: f32,
    break_ready: f32,
    place_ready: f32,
    tool_ready: f32,
}

impl FakeBody {
    fn spawn(id: u64, world: Arc<Mutex<Shared>>) -> Self {
        let eye = [id as f64 * 0.35, CELL_METRES as f64 + EYE, 0.5];
        let body = Self {
            id,
            world,
            eye,
            velocity: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            grounded: true,
            inventory: vec![(material(PICK), 1), (material(DIRT), 8)],
            total: 9,
            seen: 0,
            time: 0.0,
            break_ready: 0.0,
            place_ready: 0.0,
            tool_ready: 0.0,
        };
        body.publish();
        body
    }

    fn publish(&self) {
        let mut world = lock(&self.world);
        world.poses.insert(self.id, self.eye);
    }

    fn count_of(&self, key: u64) -> u32 {
        self.inventory
            .iter()
            .find(|(m, _)| m.key == key)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    }

    fn give(&mut self, key: u64, n: u32, events: &mut Vec<BodyEvent>) {
        if n == 0 || key == crate::facts::AIR {
            return;
        }
        if let Some(slot) = self.inventory.iter_mut().find(|(m, _)| m.key == key) {
            slot.1 = slot.1.saturating_add(n);
        } else {
            self.inventory.push((material(key), n));
        }
        self.total = self.total.saturating_add(n);
        events.push(BodyEvent::Inventory {
            material: material(key),
            delta: n as i32,
        });
    }

    fn take(&mut self, key: u64, n: u32, events: &mut Vec<BodyEvent>) {
        let Some(slot) = self.inventory.iter_mut().find(|(m, _)| m.key == key) else {
            return;
        };
        let drop_n = n.min(slot.1);
        slot.1 -= drop_n;
        self.total = self.total.saturating_sub(drop_n);
        if drop_n > 0 {
            events.push(BodyEvent::Inventory {
                material: material(key),
                delta: -(drop_n as i32),
            });
        }
    }
}

impl Drop for FakeBody {
    fn drop(&mut self) {
        lock(&self.world).poses.remove(&self.id);
    }
}

impl Body for FakeBody {
    fn sense(&mut self, rays: &[RayDir], range: f32, radius: f32, out: &mut Snapshot) {
        out.rays.clear();
        out.peers.clear();
        out.inventory.clear();
        let world = lock(&self.world);
        let (forward, right) = heading(self.yaw);
        out.pose = Some(Pose {
            eye: self.eye,
            velocity: self.velocity,
            up: Some([0.0, 1.0, 0.0]),
            grounded: self.grounded,
            in_air: true,
        });
        out.yaw = self.yaw;
        out.pitch = self.pitch;
        out.body_velocity = [
            (self.velocity[0] as f64 * right[0] + self.velocity[2] as f64 * right[1]) as f32,
            self.velocity[1],
            (self.velocity[0] as f64 * forward[0] + self.velocity[2] as f64 * forward[1]) as f32,
        ];
        out.gravity = GRAVITY;
        out.time_of_day = 0.3;
        for dir in rays {
            let aim = ray_dir(self.yaw, self.pitch, dir);
            match march(|cell| world.solid(cell), self.eye, aim, range as f64) {
                Some((dist, cell, _)) => out.rays.push(RayHit {
                    distance: dist as f32,
                    material: material(world.get(cell)),
                }),
                None => out.rays.push(RayHit {
                    distance: range,
                    material: Material::AIR,
                }),
            }
        }
        let look = ray_dir(self.yaw, self.pitch, &RayDir::view(0.0, 0.0));
        out.target =
            march(|cell| world.solid(cell), self.eye, look, 6.0).map(|(dist, cell, prev)| Target {
                cell,
                distance: dist as f32,
                material: material(world.get(cell)),
                placeable: !world.solid(prev),
            });
        out.inventory.clone_from(&self.inventory);
        out.inventory_total = self.total;
        out.inventory_capacity = 64;
        let mut peers = Vec::new();
        for (id, eye) in &world.poses {
            if *id == self.id {
                continue;
            }
            let dx = eye[0] - self.eye[0];
            let dy = eye[1] - self.eye[1];
            let dz = eye[2] - self.eye[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt() as f32;
            if distance > radius {
                continue;
            }
            peers.push(PeerView {
                offset: [
                    (dx * right[0] + dz * right[1]) as f32,
                    dy as f32,
                    (dx * forward[0] + dz * forward[1]) as f32,
                ],
                distance,
            });
        }
        drop(world);
        peers.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        out.peers = peers;
    }

    fn step(&mut self, controls: &Controls, dt: f32, radius: f32, events: &mut Vec<BodyEvent>) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.time += dt;
        self.yaw += controls.turn;
        self.pitch = (self.pitch + controls.tilt).clamp(-1.2, 1.2);
        let speed = if controls.sneak {
            SNEAK
        } else if controls.sprint {
            SPRINT
        } else {
            WALK
        };
        let (forward, right) = heading(self.yaw);
        self.velocity[0] = ((forward[0] * controls.forward as f64
            + right[0] * controls.right as f64)
            * speed as f64) as f32;
        self.velocity[2] = ((forward[1] * controls.forward as f64
            + right[1] * controls.right as f64)
            * speed as f64) as f32;
        if self.grounded && controls.jump {
            self.velocity[1] = JUMP;
            self.grounded = false;
        }
        self.velocity[1] -= GRAVITY * dt;
        {
            let world_arc = Arc::clone(&self.world);
            let world = lock(&world_arc);
            let _moved_x = slide(self, &world, self.velocity[0] as f64 * dt as f64, 0.0, 0.0);
            let moved_y = slide(self, &world, 0.0, self.velocity[1] as f64 * dt as f64, 0.0);
            if !moved_y && self.velocity[1] <= 0.0 {
                let feet_y = self.eye[1] - EYE;
                let below = cell_at([self.eye[0], feet_y - 0.02, self.eye[2]]);
                if world.solid(below) {
                    let surface = (below.1 as f64 + 1.0) * CELL_METRES as f64;
                    self.eye[1] = surface + EYE;
                    self.velocity[1] = 0.0;
                    self.grounded = true;
                }
            } else if moved_y {
                self.grounded = false;
            }
            let _ = slide(self, &world, 0.0, 0.0, self.velocity[2] as f64 * dt as f64);
        }
        if let Some(act) = controls.act {
            self.act(act, events);
        }
        self.mutations(radius, events);
        self.publish();
    }

    fn alive(&self) -> bool {
        true
    }
}

impl FakeBody {
    fn act(&mut self, act: Act, events: &mut Vec<BodyEvent>) {
        let ready = match act {
            Act::Break => self.break_ready,
            Act::Place(_) => self.place_ready,
            Act::UseTool(_) => self.tool_ready,
            Act::Swing => 0.0,
        };
        if self.time + 1e-4 < ready {
            events.push(BodyEvent::Refused(Refusal::Cooldown));
            return;
        }
        match act {
            Act::Swing => {}
            Act::Break => self.break_cell(events),
            Act::Place(key) => self.place_cell(key, events),
            Act::UseTool(key) => self.use_tool(key, events),
        }
    }

    fn break_cell(&mut self, events: &mut Vec<BodyEvent>) {
        let mut world = lock(&self.world);
        let Some(hit) = look_hit(&world, self.eye, self.yaw, self.pitch) else {
            events.push(BodyEvent::Refused(Refusal::NoTarget));
            return;
        };
        let key = world.get(hit.0);
        if key == crate::facts::AIR {
            events.push(BodyEvent::Refused(Refusal::NoTarget));
            return;
        }
        if key == STONE || hit.0.1 <= 0 {
            events.push(BodyEvent::Refused(Refusal::Occupied));
            return;
        }
        let before = material(key);
        world.cells.remove(&hit.0);
        world.changes.push(Change {
            source: self.id,
            cell: hit.0,
            before,
            after: Material::AIR,
        });
        drop(world);
        self.break_ready = self.time + 0.25;
        events.push(edit(
            hit.0,
            EditKind::Broke,
            before,
            Material::AIR,
            Outcome::Predicted,
        ));
        events.push(edit(
            hit.0,
            EditKind::Broke,
            before,
            Material::AIR,
            Outcome::Confirmed,
        ));
        self.give(key, 1, events);
    }

    fn place_cell(&mut self, key: u64, events: &mut Vec<BodyEvent>) {
        if self.count_of(key) == 0 {
            events.push(BodyEvent::Refused(Refusal::NotHeld));
            return;
        }
        let mut world = lock(&self.world);
        let Some(hit) = look_hit(&world, self.eye, self.yaw, self.pitch) else {
            events.push(BodyEvent::Refused(Refusal::NoTarget));
            return;
        };
        if world.solid(hit.1) {
            events.push(BodyEvent::Refused(Refusal::Occupied));
            return;
        }
        let before = material(world.get(hit.1));
        let after = material(key);
        world.cells.insert(hit.1, key);
        world.changes.push(Change {
            source: self.id,
            cell: hit.1,
            before,
            after,
        });
        drop(world);
        self.place_ready = self.time + 0.20;
        events.push(edit(
            hit.1,
            EditKind::Placed,
            before,
            after,
            Outcome::Predicted,
        ));
        events.push(edit(
            hit.1,
            EditKind::Placed,
            before,
            after,
            Outcome::Confirmed,
        ));
        self.take(key, 1, events);
    }

    fn use_tool(&mut self, key: u64, events: &mut Vec<BodyEvent>) {
        if self.count_of(key) == 0 {
            events.push(BodyEvent::Refused(Refusal::NotHeld));
            return;
        }
        let mut world = lock(&self.world);
        let Some(hit) = look_hit(&world, self.eye, self.yaw, self.pitch) else {
            events.push(BodyEvent::Refused(Refusal::NoTarget));
            return;
        };
        let before_key = world.get(hit.0);
        let before = material(before_key);
        let tool = material(key);
        let mut after = before;
        let mut reacted = false;
        if key == PICK && before_key == ORE {
            world.cells.remove(&hit.0);
            after = Material::AIR;
            reacted = true;
            world.changes.push(Change {
                source: self.id,
                cell: hit.0,
                before,
                after,
            });
        }
        drop(world);
        self.tool_ready = self.time + 0.25;
        events.push(BodyEvent::Tool {
            cell: hit.0,
            cell_before: before,
            tool_before: tool,
            cell_after: after,
            tool_after: tool,
            reacted,
        });
        if reacted {
            self.give(GEM, 1, events);
        }
    }

    fn mutations(&mut self, radius: f32, events: &mut Vec<BodyEvent>) {
        let world_arc = Arc::clone(&self.world);
        let world = lock(&world_arc);
        let end = world.changes.len();
        for change in world.changes.iter().take(end).skip(self.seen) {
            if change.source == self.id {
                continue;
            }
            let distance = cell_distance(self.eye, change.cell);
            if distance <= radius && change.before.key != change.after.key {
                events.push(BodyEvent::Mutation {
                    cell: change.cell,
                    before: change.before,
                    after: change.after,
                    distance,
                });
            }
        }
        self.seen = end;
    }
}

fn edit(
    cell: Cell,
    kind: EditKind,
    before: Material,
    after: Material,
    outcome: Outcome,
) -> BodyEvent {
    BodyEvent::Edit {
        cell,
        kind,
        before,
        after,
        outcome,
    }
}

fn look_hit(world: &Shared, eye: [f64; 3], yaw: f32, pitch: f32) -> Option<(Cell, Cell)> {
    let dir = ray_dir(yaw, pitch, &RayDir::view(0.0, 0.0));
    march(|cell| world.solid(cell), eye, dir, 6.0).map(|(_, cell, prev)| (cell, prev))
}

fn slide(body: &mut FakeBody, world: &Shared, dx: f64, dy: f64, dz: f64) -> bool {
    let next = [body.eye[0] + dx, body.eye[1] + dy, body.eye[2] + dz];
    let feet = [next[0], next[1] - EYE, next[2]];
    if world.solid(cell_at(feet)) {
        if dx != 0.0 {
            body.velocity[0] = 0.0;
        }
        if dz != 0.0 {
            body.velocity[2] = 0.0;
        }
        return false;
    }
    body.eye = next;
    true
}

fn heading(yaw: f32) -> ([f64; 2], [f64; 2]) {
    let (sy, cy) = yaw.sin_cos();
    ([-sy as f64, cy as f64], [cy as f64, sy as f64])
}

fn ray_dir(yaw: f32, pitch: f32, dir: &RayDir) -> [f64; 3] {
    match dir.basis {
        RayBasis::Up => [0.0, 1.0, 0.0],
        RayBasis::Down => [0.0, -1.0, 0.0],
        RayBasis::View => {
            let (sy, cy) = (yaw + dir.yaw).sin_cos();
            let (sp, cp) = (pitch + dir.pitch).sin_cos();
            [-(sy as f64) * cp as f64, sp as f64, cy as f64 * cp as f64]
        }
    }
}

fn cell_at(p: [f64; 3]) -> Cell {
    let s = CELL_METRES as f64;
    (
        (p[0] / s).floor() as i32,
        (p[1] / s).floor() as i32,
        (p[2] / s).floor() as i32,
    )
}

fn cell_distance(eye: [f64; 3], cell: Cell) -> f32 {
    let s = CELL_METRES as f64;
    let c = [
        (cell.0 as f64 + 0.5) * s,
        (cell.1 as f64 + 0.5) * s,
        (cell.2 as f64 + 0.5) * s,
    ];
    let dx = eye[0] - c[0];
    let dy = eye[1] - c[1];
    let dz = eye[2] - c[2];
    (dx * dx + dy * dy + dz * dz).sqrt() as f32
}

/// Grid march. Distance is metres. `previous` is the last empty cell.
fn march(
    solid: impl Fn(Cell) -> bool,
    origin: [f64; 3],
    dir: [f64; 3],
    range: f64,
) -> Option<(f64, Cell, Cell)> {
    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if len < 1e-8 || !range.is_finite() || range < 0.0 {
        return None;
    }
    let dir = [dir[0] / len, dir[1] / len, dir[2] / len];
    let scale = CELL_METRES as f64;
    let origin = [origin[0] / scale, origin[1] / scale, origin[2] / scale];
    let reach = range / scale;
    let mut cell = (
        origin[0].floor() as i32,
        origin[1].floor() as i32,
        origin[2].floor() as i32,
    );
    if solid(cell) {
        return Some((0.0, cell, cell));
    }
    let step = [
        if dir[0] >= 0.0 { 1i32 } else { -1 },
        if dir[1] >= 0.0 { 1 } else { -1 },
        if dir[2] >= 0.0 { 1 } else { -1 },
    ];
    let delta = [
        if dir[0].abs() < 1e-12 {
            f64::INFINITY
        } else {
            (1.0 / dir[0]).abs()
        },
        if dir[1].abs() < 1e-12 {
            f64::INFINITY
        } else {
            (1.0 / dir[1]).abs()
        },
        if dir[2].abs() < 1e-12 {
            f64::INFINITY
        } else {
            (1.0 / dir[2]).abs()
        },
    ];
    let mut tmax = [0.0; 3];
    let cell_f = [cell.0 as f64, cell.1 as f64, cell.2 as f64];
    for axis in 0..3 {
        tmax[axis] = if delta[axis].is_infinite() {
            f64::INFINITY
        } else if step[axis] > 0 {
            (cell_f[axis] + 1.0 - origin[axis]) * delta[axis]
        } else {
            (origin[axis] - cell_f[axis]) * delta[axis]
        };
    }
    let mut prev;
    for _ in 0..64 {
        let axis = if tmax[0] <= tmax[1] && tmax[0] <= tmax[2] {
            0
        } else if tmax[1] <= tmax[2] {
            1
        } else {
            2
        };
        let t = tmax[axis];
        if t > reach {
            return None;
        }
        prev = cell;
        match axis {
            0 => cell.0 += step[0],
            1 => cell.1 += step[1],
            _ => cell.2 += step[2],
        }
        tmax[axis] += delta[axis];
        if solid(cell) {
            return Some((t * scale, cell, prev));
        }
    }
    None
}

fn lock(world: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    world.lock().unwrap_or_else(|err| err.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::senses;
    use crate::store;
    use crate::swarm::{self, Config, ServerArgs};
    use std::fs;
    use std::path::Path;
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn breaking_ore_grants_a_new_material_and_then_cools_down() {
        let connect = FakeConnect::new();
        let mut body = connect.connect("swarm01").unwrap();
        let mut snap = Snapshot::default();
        body.sense(
            &senses::ray_table(),
            senses::RAY_RANGE,
            senses::SENSE_RADIUS,
            &mut snap,
        );
        let target = snap.target.expect("ore in front");
        assert_eq!(target.material.key, ORE);
        assert!(target.distance < 6.0, "{}", target.distance);
        assert!(snap.pose.is_some_and(|p| p.grounded && p.up.is_some()));
        let mut events = Vec::new();
        body.step(
            &Controls {
                act: Some(Act::Break),
                ..Controls::default()
            },
            1.0 / 30.0,
            32.0,
            &mut events,
        );
        assert!(events.iter().any(|ev| matches!(
            ev,
            BodyEvent::Edit {
                outcome: Outcome::Confirmed,
                kind: EditKind::Broke,
                ..
            }
        )));
        assert!(events.iter().any(|ev| matches!(
            ev,
            BodyEvent::Inventory { material, delta } if material.key == ORE && *delta > 0
        )));
        events.clear();
        body.step(
            &Controls {
                act: Some(Act::Break),
                ..Controls::default()
            },
            1.0 / 30.0,
            32.0,
            &mut events,
        );
        assert!(
            events
                .iter()
                .any(|ev| matches!(ev, BodyEvent::Refused(Refusal::Cooldown)))
        );
    }

    #[test]
    fn a_short_swarm_learns_a_few_cells_and_obeys_control() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("xswarm-e2e-{stamp}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let world = FakeConnect::new();
        let mut cfg = Config::new(
            ServerArgs {
                host: "fake".into(),
                port: 1,
                password: String::new(),
            },
            "explorer",
            dir.clone(),
        );
        cfg.agents = 3;
        cfg.decisions = Some(30);
        cfg.temperature = 4.0;
        cfg.reset_every = 1;
        cfg.console = false;
        cfg.fresh = true;
        cfg.paced = false;
        cfg.seed = 7;
        cfg.reeval = 0.0;
        cfg.experience = 8;
        cfg.poll_every = Duration::from_millis(30);
        cfg.status_every = Duration::from_millis(60);
        cfg.checkpoint_every = Duration::from_millis(400);
        let connect = world.clone();
        let handle = thread::spawn(move || swarm::launch(cfg, Box::new(connect)));
        let deadline = Instant::now() + Duration::from_secs(25);
        let ready = loop {
            if handle.is_finished() {
                panic!("swarm stopped before it had learned: {:?}", handle.join());
            }
            if let Ok(status) = store::load_status(&dir.join(store::STATUS_FILE)) {
                let cells = status.coverage * 4096.0;
                if status.discoveries >= 1 && status.episodes >= 6 && cells >= 3.0 {
                    break status;
                }
            }
            if Instant::now() > deadline {
                panic!("timed out waiting for the archive\n{}", dump(&dir));
            }
            thread::sleep(Duration::from_millis(20));
        };
        let _ = ready;
        send(&dir, "save");
        assert!(dir.join(store::ARCHIVE_FILE).is_file(), "{}", dump(&dir));
        send(&dir, "pause");
        thread::sleep(Duration::from_millis(120));
        let paused = store::load_status(&dir.join(store::STATUS_FILE)).expect("status");
        assert!(paused.paused, "{}", dump(&dir));
        let mark = paused.episodes;
        thread::sleep(Duration::from_millis(300));
        let held = refresh(&dir);
        assert_eq!(held.episodes, mark, "pause kept evaluating\n{}", dump(&dir));
        send(&dir, "resume");
        let resumed = refresh(&dir);
        assert!(!resumed.paused);
        send(&dir, "agents 1");
        let slim = wait_agents(&dir, &handle, 1, Instant::now() + Duration::from_secs(8));
        assert!(slim.agents <= 1, "agents={}\n{}", slim.agents, dump(&dir));
        send(&dir, "quit");
        let code = join_swarm(handle, &dir);
        assert_eq!(code, 0, "{}", dump(&dir));
        let (archive, _) = store::load_archive(&dir.join(store::ARCHIVE_FILE)).unwrap();
        assert!(
            archive.elite_count() >= 3,
            "elites {}\n{}",
            archive.elite_count(),
            dump(&dir)
        );
        let log = fs::read_to_string(dir.join(store::DISCOVERY_LOG)).unwrap_or_default();
        assert!(log.contains('\t'), "discovery log:\n{log}");
        assert!(world.connects() > 3, "connects {}", world.connects());

        let before = archive.episodes;
        let mut again = Config::new(
            ServerArgs {
                host: "fake".into(),
                port: 1,
                password: String::new(),
            },
            "explorer",
            dir.clone(),
        );
        again.agents = 1;
        again.decisions = Some(20);
        again.console = false;
        again.fresh = false;
        again.paced = false;
        again.reeval = 0.0;
        again.temperature = 2.0;
        again.poll_every = Duration::from_millis(30);
        again.status_every = Duration::from_millis(60);
        again.checkpoint_every = Duration::from_secs(30);
        let connect = world.clone();
        let handle = thread::spawn(move || swarm::launch(again, Box::new(connect)));
        let limit = Instant::now() + Duration::from_secs(15);
        loop {
            if handle.is_finished() {
                panic!("resume run exited early: {:?}{}", handle.join(), dump(&dir));
            }
            if let Ok(status) = store::load_status(&dir.join(store::STATUS_FILE))
                && status.episodes > before
            {
                break;
            }
            if Instant::now() > limit {
                panic!("resume did not add episodes\n{}", dump(&dir));
            }
            thread::sleep(Duration::from_millis(20));
        }
        send(&dir, "quit");
        assert_eq!(join_swarm(handle, &dir), 0);
        let (archive, _) = store::load_archive(&dir.join(store::ARCHIVE_FILE)).unwrap();
        assert!(archive.elite_count() >= 3);
        assert!(archive.episodes > before);
        let _ = fs::remove_dir_all(&dir);
    }

    fn send(dir: &Path, command: &str) {
        let path = dir.join(store::CONTROL_FILE);
        fs::write(&path, format!("{command}\n")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let text = fs::read_to_string(&path).unwrap_or_default();
            if text.trim().is_empty() {
                return;
            }
            thread::sleep(Duration::from_millis(15));
        }
        panic!(
            "control {command:?} was not consumed:\n{}",
            fs::read_to_string(&path).unwrap_or_default()
        );
    }

    fn refresh(dir: &Path) -> store::Status {
        send(dir, "status");
        store::load_status(&dir.join(store::STATUS_FILE))
            .unwrap_or_else(|err| panic!("{err}\n{}", dump(dir)))
    }

    fn wait_agents(
        dir: &Path,
        handle: &thread::JoinHandle<i32>,
        cap: u32,
        deadline: Instant,
    ) -> store::Status {
        loop {
            if handle.is_finished() {
                panic!("swarm exited while waiting for agents\n{}", dump(dir));
            }
            let status = refresh(dir);
            if status.agents <= cap {
                return status;
            }
            if Instant::now() > deadline {
                panic!("agents stayed at {}\n{}", status.agents, dump(dir));
            }
        }
    }

    fn join_swarm(handle: thread::JoinHandle<i32>, dir: &Path) -> i32 {
        let start = Instant::now();
        while !handle.is_finished() {
            if start.elapsed() > Duration::from_secs(15) {
                panic!("swarm did not exit\n{}", dump(dir));
            }
            thread::sleep(Duration::from_millis(20));
        }
        handle.join().expect("swarm thread")
    }

    fn dump(dir: &Path) -> String {
        let status =
            fs::read_to_string(dir.join(store::STATUS_FILE)).unwrap_or_else(|err| err.to_string());
        let episodes = fs::read_to_string(dir.join(store::EPISODES_LOG)).unwrap_or_default();
        let tail: String = episodes
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        format!("status:\n{status}\nepisodes:\n{tail}")
    }
}
