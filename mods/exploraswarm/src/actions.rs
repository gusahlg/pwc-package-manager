// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The discrete actions a player can take, and the per-episode hand slots they name.
//!
//! Twenty-two actions. Movement and looking are held on every physics frame until the next
//! decision. A world act (break, place, use, swing) is issued once, on the decision's first frame.
//! Looking turns at 180°/s: `turn` and `tilt` are the radians to apply this frame, so the caller
//! passes `dt`. Positive `turn` yaws toward the body's left; positive `tilt` looks up.
//!
//! Hand slots: four material keys, filled from the inventory in inventory order at episode start.
//! A material gained later takes the first empty slot. A slot keeps its key for the whole episode,
//! so `Place(k)` means the same material throughout. When the count hits zero the slot is masked.

use crate::body::{Act, Controls};
use crate::facts::{AIR, ActionKind, Material};

pub const ACTION_COUNT: usize = 22;
pub const HAND_SLOTS: usize = 4;

/// World acts are masked until this long after the previous one (8 per second).
pub const WORLD_GAP: f32 = 0.125;
pub const COOLDOWN_BREAK: f32 = 0.25;
pub const COOLDOWN_PLACE: f32 = 0.20;
pub const COOLDOWN_TOOL: f32 = 0.25;

const TURN_RATE: f32 = std::f32::consts::PI;

pub const IDLE: u16 = 0;
pub const FORWARD: u16 = 1;
pub const BACK: u16 = 2;
pub const LEFT: u16 = 3;
pub const RIGHT: u16 = 4;
pub const FORWARD_JUMP: u16 = 5;
pub const SPRINT: u16 = 6;
pub const SNEAK: u16 = 7;
pub const TURN_LEFT: u16 = 8;
pub const TURN_RIGHT: u16 = 9;
pub const LOOK_UP: u16 = 10;
pub const LOOK_DOWN: u16 = 11;
pub const BREAK: u16 = 12;
pub const PLACE0: u16 = 13;
pub const USE0: u16 = 17;
pub const SWING: u16 = 21;

pub const NAMES: [&str; ACTION_COUNT] = [
    "idle",
    "forward",
    "back",
    "left",
    "right",
    "forward-jump",
    "sprint",
    "sneak",
    "turn-left",
    "turn-right",
    "look-up",
    "look-down",
    "break",
    "place0",
    "place1",
    "place2",
    "place3",
    "use0",
    "use1",
    "use2",
    "use3",
    "swing",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Idle,
    Forward,
    Back,
    Left,
    Right,
    ForwardJump,
    Sprint,
    Sneak,
    TurnLeft,
    TurnRight,
    LookUp,
    LookDown,
    Break,
    Place(u8),
    UseTool(u8),
    Swing,
}

pub fn decode(action: u16) -> Action {
    match action {
        FORWARD => Action::Forward,
        BACK => Action::Back,
        LEFT => Action::Left,
        RIGHT => Action::Right,
        FORWARD_JUMP => Action::ForwardJump,
        SPRINT => Action::Sprint,
        SNEAK => Action::Sneak,
        TURN_LEFT => Action::TurnLeft,
        TURN_RIGHT => Action::TurnRight,
        LOOK_UP => Action::LookUp,
        LOOK_DOWN => Action::LookDown,
        BREAK => Action::Break,
        13..=16 => Action::Place((action - PLACE0) as u8),
        17..=20 => Action::UseTool((action - USE0) as u8),
        SWING => Action::Swing,
        _ => Action::Idle,
    }
}

pub fn kind_of(action: u16) -> ActionKind {
    match decode(action) {
        Action::Idle => ActionKind::Idle,
        Action::Forward
        | Action::Back
        | Action::Left
        | Action::Right
        | Action::Sprint
        | Action::Sneak => ActionKind::Move,
        Action::ForwardJump => ActionKind::Jump,
        Action::TurnLeft | Action::TurnRight | Action::LookUp | Action::LookDown => {
            ActionKind::Look
        }
        Action::Break => ActionKind::Break,
        Action::Place(_) => ActionKind::Place,
        Action::UseTool(_) => ActionKind::UseTool,
        Action::Swing => ActionKind::Swing,
    }
}

/// Four slots that stay put for one episode.
#[derive(Clone, Debug)]
pub struct HandTable {
    pub keys: [u64; HAND_SLOTS],
    pub counts: [u32; HAND_SLOTS],
}

impl Default for HandTable {
    fn default() -> Self {
        Self {
            keys: [AIR; HAND_SLOTS],
            counts: [0; HAND_SLOTS],
        }
    }
}

impl HandTable {
    /// Fill from the inventory, in the order it is listed. Duplicate keys stack into the earlier slot.
    pub fn begin(&mut self, inventory: &[(Material, u32)]) {
        self.keys = [AIR; HAND_SLOTS];
        self.counts = [0; HAND_SLOTS];
        for (mat, count) in inventory {
            if mat.is_air() || *count == 0 {
                continue;
            }
            if let Some(i) = self.keys.iter().position(|&k| k == mat.key) {
                self.counts[i] = self.counts[i].saturating_add(*count);
                continue;
            }
            if let Some(i) = self.keys.iter().position(|&k| k == AIR) {
                self.keys[i] = mat.key;
                self.counts[i] = *count;
            }
        }
    }

    /// A newly obtained material takes the first empty slot. Keys already held are left alone.
    pub fn note_gained(&mut self, material: &Material, delta: i32) {
        if delta <= 0 || material.is_air() {
            return;
        }
        if self.keys.contains(&material.key) {
            return;
        }
        if let Some(i) = self.keys.iter().position(|&k| k == AIR) {
            self.keys[i] = material.key;
            self.counts[i] = delta as u32;
        }
    }

    /// Refresh counts from the current inventory. Keys do not move.
    pub fn sync(&mut self, inventory: &[(Material, u32)]) {
        for i in 0..HAND_SLOTS {
            if self.keys[i] == AIR {
                self.counts[i] = 0;
                continue;
            }
            let key = self.keys[i];
            self.counts[i] = inventory
                .iter()
                .filter(|(m, _)| m.key == key)
                .map(|(_, c)| *c)
                .sum();
        }
    }

    pub fn key(&self, slot: u8) -> u64 {
        self.keys.get(slot as usize).copied().unwrap_or(AIR)
    }

    pub fn count(&self, slot: u8) -> u32 {
        self.counts.get(slot as usize).copied().unwrap_or(0)
    }
}

/// When the next world act of each kind is allowed, on the episode clock.
#[derive(Clone, Copy, Debug)]
pub struct Cooldowns {
    pub now: f32,
    /// Episode time of the last world act, or a negative value when there has not been one.
    pub last_world: f32,
    pub break_ready: f32,
    pub place_ready: f32,
    pub tool_ready: f32,
}

impl Default for Cooldowns {
    fn default() -> Self {
        Self {
            now: 0.0,
            last_world: -1.0,
            break_ready: 0.0,
            place_ready: 0.0,
            tool_ready: 0.0,
        }
    }
}

impl Cooldowns {
    pub fn world_ready(&self) -> bool {
        self.last_world < 0.0 || self.now + 1e-4 >= self.last_world + WORLD_GAP
    }

    pub fn on_world_act(&mut self, kind: ActionKind) {
        self.last_world = self.now;
        match kind {
            ActionKind::Break => self.break_ready = self.now + COOLDOWN_BREAK,
            ActionKind::Place => self.place_ready = self.now + COOLDOWN_PLACE,
            ActionKind::UseTool => self.tool_ready = self.now + COOLDOWN_TOOL,
            _ => {}
        }
    }
}

/// Write the legality of every action. Idle is always legal.
pub fn mask(legal: &mut [bool], target: bool, placeable: bool, hands: &HandTable, cd: &Cooldowns) {
    for slot in legal.iter_mut() {
        *slot = false;
    }
    if legal.is_empty() {
        return;
    }
    legal[0] = true;
    for a in 1..ACTION_COUNT.min(legal.len()) {
        legal[a] = action_legal(a as u16, target, placeable, hands, cd);
    }
}

fn action_legal(
    action: u16,
    target: bool,
    placeable: bool,
    hands: &HandTable,
    cd: &Cooldowns,
) -> bool {
    match decode(action) {
        Action::Idle
        | Action::Forward
        | Action::Back
        | Action::Left
        | Action::Right
        | Action::ForwardJump
        | Action::Sprint
        | Action::Sneak
        | Action::TurnLeft
        | Action::TurnRight
        | Action::LookUp
        | Action::LookDown => true,
        Action::Break => target && cd.world_ready() && cd.now + 1e-4 >= cd.break_ready,
        Action::Place(k) => {
            placeable && hands.count(k) > 0 && cd.world_ready() && cd.now + 1e-4 >= cd.place_ready
        }
        Action::UseTool(k) => {
            target && hands.count(k) > 0 && cd.world_ready() && cd.now + 1e-4 >= cd.tool_ready
        }
        Action::Swing => cd.world_ready(),
    }
}

/// Bit `i` is set when action `i` is legal. The 22 actions fit in one `u64`.
pub fn mask_bits(legal: &[bool]) -> u64 {
    let mut bits = 0u64;
    for (i, &on) in legal.iter().enumerate().take(64) {
        if on {
            bits |= 1u64 << i;
        }
    }
    bits
}

pub fn legal_count(legal: &[bool]) -> u16 {
    legal.iter().filter(|on| **on).count() as u16
}

/// Controls held until the next decision. `issue_act` is true only on the decision's first frame.
pub fn controls(action: u16, dt: f32, hands: &HandTable, issue_act: bool) -> Controls {
    let mut c = Controls::default();
    let step = TURN_RATE * dt.max(0.0);
    match decode(action) {
        Action::Idle => {}
        Action::Forward => c.forward = 1.0,
        Action::Back => c.forward = -1.0,
        Action::Left => c.right = -1.0,
        Action::Right => c.right = 1.0,
        Action::ForwardJump => {
            c.forward = 1.0;
            c.jump = true;
        }
        Action::Sprint => {
            c.forward = 1.0;
            c.sprint = true;
        }
        Action::Sneak => {
            c.forward = 1.0;
            c.sneak = true;
        }
        Action::TurnLeft => c.turn = step,
        Action::TurnRight => c.turn = -step,
        Action::LookUp => c.tilt = step,
        Action::LookDown => c.tilt = -step,
        Action::Break => {
            if issue_act {
                c.act = Some(Act::Break);
            }
        }
        Action::Place(k) => {
            let key = hands.key(k);
            if issue_act && key != AIR {
                c.act = Some(Act::Place(key));
            }
        }
        Action::UseTool(k) => {
            let key = hands.key(k);
            if issue_act && key != AIR {
                c.act = Some(Act::UseTool(key));
            }
        }
        Action::Swing => {
            if issue_act {
                c.act = Some(Act::Swing);
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::MATERIAL_FEATURES;

    fn mat(key: u64) -> Material {
        let mut features = [0.0; MATERIAL_FEATURES];
        features[0] = 1.0;
        Material { key, features }
    }

    #[test]
    fn hand_slots_stay_put_for_the_episode() {
        let mut hands = HandTable::default();
        hands.begin(&[(mat(10), 3), (mat(20), 1), (mat(10), 2), (mat(30), 4)]);
        assert_eq!(hands.keys, [10, 20, 30, AIR]);
        assert_eq!(
            hands.counts[0], 5,
            "the second stack of key 10 adds to the first slot"
        );
        hands.note_gained(&mat(40), 2);
        assert_eq!(hands.keys[3], 40);
        hands.note_gained(&mat(50), 1);
        assert_eq!(
            hands.keys,
            [10, 20, 30, 40],
            "no empty slot, so 50 is not taken"
        );
        hands.sync(&[(mat(10), 0), (mat(40), 7)]);
        assert_eq!(hands.keys, [10, 20, 30, 40], "keys survive a zero count");
        assert_eq!(hands.counts, [0, 0, 0, 7]);
    }

    #[test]
    fn mask_respects_target_held_cooldowns_and_the_rate_cap() {
        let mut hands = HandTable::default();
        hands.begin(&[(mat(10), 2), (mat(20), 1)]);
        let mut legal = [false; ACTION_COUNT];
        let mut cd = Cooldowns::default();

        mask(&mut legal, false, false, &hands, &cd);
        assert!(legal[IDLE as usize]);
        assert!(!legal[BREAK as usize]);
        assert!(!legal[PLACE0 as usize], "nothing to place against");
        assert!(!legal[USE0 as usize], "no target");
        assert!(legal[SWING as usize]);

        mask(&mut legal, true, true, &hands, &cd);
        assert!(legal[BREAK as usize]);
        assert!(legal[PLACE0 as usize]);
        assert!(legal[(PLACE0 + 1) as usize]);
        assert!(!legal[(PLACE0 + 2) as usize], "slot 2 is empty");
        assert!(legal[USE0 as usize]);

        cd.on_world_act(ActionKind::Break);
        mask(&mut legal, true, true, &hands, &cd);
        assert!(!legal[BREAK as usize]);
        assert!(
            !legal[PLACE0 as usize],
            "the shared rate cap blocks every world act"
        );
        assert!(!legal[SWING as usize]);
        assert!(legal[FORWARD as usize]);

        cd.now = 0.20;
        mask(&mut legal, true, true, &hands, &cd);
        assert!(!legal[BREAK as usize], "break's own cooldown is 0.25 s");
        assert!(
            legal[PLACE0 as usize],
            "place's cooldown was not started, and the rate gap has passed"
        );
        assert!(legal[SWING as usize]);

        cd.on_world_act(ActionKind::Place);
        cd.now = 0.33;
        mask(&mut legal, true, true, &hands, &cd);
        assert!(
            legal[BREAK as usize],
            "break's cooldown elapsed and the rate gap has passed"
        );
        assert!(!legal[PLACE0 as usize], "place's own cooldown runs to 0.40");

        cd.now = 0.41;
        mask(&mut legal, true, true, &hands, &cd);
        assert!(legal[BREAK as usize]);
        assert!(legal[PLACE0 as usize]);

        hands.counts[0] = 0;
        mask(&mut legal, true, true, &hands, &cd);
        assert!(!legal[PLACE0 as usize]);
        assert!(!legal[USE0 as usize]);
        assert!(legal[IDLE as usize]);
    }

    #[test]
    fn world_act_is_only_on_the_first_frame() {
        let mut hands = HandTable::default();
        hands.begin(&[(mat(9), 1)]);
        let first = controls(BREAK, 0.033, &hands, true);
        assert_eq!(first.act, Some(Act::Break));
        let later = controls(BREAK, 0.033, &hands, false);
        assert_eq!(later.act, None);
        let look = controls(TURN_LEFT, 0.5, &hands, true);
        assert!((look.turn - std::f32::consts::PI * 0.5).abs() < 1e-5);
        let sneak = controls(SNEAK, 0.033, &hands, true);
        assert_eq!(sneak.forward, 1.0);
        assert!(sneak.sneak);
    }
}
