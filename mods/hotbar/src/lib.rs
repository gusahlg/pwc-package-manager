//! The hotbar: nine slots of held materials plus the bare hand, along the bottom of the screen.
//!
//! A slot names a stack in the core inventory. The selected slot is what the player *holds*: a left
//! click with it runs the law between it and the targeted block (a tool is just a block in hand),
//! a right click places one unit of it. The bare hand (slot 0) breaks blocks. Keys 1-9 select a
//! slot (pressing the selected one again returns to the hand), 0 selects the hand, the wheel
//! cycles. Newly gathered materials drop into the first empty slot; the inventory (press I)
//! equips any held material into any slot. When a tool reaction changes the held unit, the slot
//! follows it to its new configuration.
//!
//! # Shared handles
//!
//! [`register`] provides two handles through the [`ModRegistrar`]: [`HotbarHandle`] (the slots
//! and the selection) and [`ItemUiHandle`] (whether an item panel such as the inventory is open
//! and owns the number keys and the wheel). A package that depends on `pwc.hotbar` reads them
//! back with `registrar.get::<HotbarHandle>()` and `registrar.get::<ItemUiHandle>()`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use pwc_mod_api::block::{BlockId, AIR};
use pwc_mod_api::derived::Memo;
use pwc_mod_api::engine::{Color, Key};
use pwc_mod_api::input::intent::{Chord, Source};
use pwc_mod_api::player::Player;
use pwc_mod_api::ui::{Anchor, HudElement, Role};
use pwc_mod_api::world::World;
use pwc_mod_api::{Action, Mod, ModContext, ModRegistrar, ToolUse, ESSENTIALS};

/// Material slots (keys 1-9); slot 0 is the bare hand.
pub const SLOTS: usize = 9;

/// Action ids. The inventory equip path uses [`SLOT_IDS`].
pub const HAND_ID: &str = "hotbar.hand";
pub const SLOT_IDS: [&str; SLOTS] = [
    "hotbar.slot1",
    "hotbar.slot2",
    "hotbar.slot3",
    "hotbar.slot4",
    "hotbar.slot5",
    "hotbar.slot6",
    "hotbar.slot7",
    "hotbar.slot8",
    "hotbar.slot9",
];
pub const NEXT_ID: &str = "hotbar.next";
pub const PREV_ID: &str = "hotbar.prev";

const HAND_CHORD: &[Chord] = &[Chord::key(Key::Num0)];
const SLOT_CHORDS: [&[Chord]; SLOTS] = [
    &[Chord::key(Key::Num1)],
    &[Chord::key(Key::Num2)],
    &[Chord::key(Key::Num3)],
    &[Chord::key(Key::Num4)],
    &[Chord::key(Key::Num5)],
    &[Chord::key(Key::Num6)],
    &[Chord::key(Key::Num7)],
    &[Chord::key(Key::Num8)],
    &[Chord::key(Key::Num9)],
];
const NEXT_CHORD: &[Chord] = &[Chord::bare(Source::WheelDown)];
const PREV_CHORD: &[Chord] = &[Chord::bare(Source::WheelUp)];

const ACTIONS: &[Action] = &[
    Action { id: HAND_ID, label: "Hand", default: HAND_CHORD, repeat: false, held: false },
    Action { id: SLOT_IDS[0], label: "Slot 1", default: SLOT_CHORDS[0], repeat: false, held: false },
    Action { id: SLOT_IDS[1], label: "Slot 2", default: SLOT_CHORDS[1], repeat: false, held: false },
    Action { id: SLOT_IDS[2], label: "Slot 3", default: SLOT_CHORDS[2], repeat: false, held: false },
    Action { id: SLOT_IDS[3], label: "Slot 4", default: SLOT_CHORDS[3], repeat: false, held: false },
    Action { id: SLOT_IDS[4], label: "Slot 5", default: SLOT_CHORDS[4], repeat: false, held: false },
    Action { id: SLOT_IDS[5], label: "Slot 6", default: SLOT_CHORDS[5], repeat: false, held: false },
    Action { id: SLOT_IDS[6], label: "Slot 7", default: SLOT_CHORDS[6], repeat: false, held: false },
    Action { id: SLOT_IDS[7], label: "Slot 8", default: SLOT_CHORDS[7], repeat: false, held: false },
    Action { id: SLOT_IDS[8], label: "Slot 9", default: SLOT_CHORDS[8], repeat: false, held: false },
    Action { id: NEXT_ID, label: "Next", default: NEXT_CHORD, repeat: false, held: false },
    Action { id: PREV_ID, label: "Previous", default: PREV_CHORD, repeat: false, held: false },
];

/// The hotbar's state, shared with the inventory (which equips into it).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct HotbarState {
    /// `slots[i]` is key `i + 1`.
    pub slots: [Option<BlockId>; SLOTS],
    /// 0 = the bare hand, 1..=9 = a slot.
    pub selected: usize,
}

impl HotbarState {
    /// The material in the selected slot, if any.
    pub fn selected_id(&self) -> Option<BlockId> {
        if self.selected == 0 { None } else { self.slots[self.selected - 1] }
    }

    /// Put `id` into slot `key` (1..=9), clearing it from any other slot.
    pub fn equip(&mut self, key: usize, id: BlockId) {
        if !(1..=SLOTS).contains(&key) {
            return;
        }
        for s in self.slots.iter_mut() {
            if *s == Some(id) {
                *s = None;
            }
        }
        self.slots[key - 1] = Some(id);
    }

    /// The key (1..=9) a material sits in, if any.
    pub fn key_of(&self, id: BlockId) -> Option<usize> {
        self.slots.iter().position(|&s| s == Some(id)).map(|i| i + 1)
    }

    /// Drop `id` into the first empty slot if it is not on the bar yet.
    fn adopt(&mut self, id: BlockId) {
        if id == AIR || self.key_of(id).is_some() {
            return;
        }
        if let Some(i) = self.slots.iter().position(|s| s.is_none()) {
            self.slots[i] = Some(id);
        }
    }

    /// Clear slots whose stack is gone.
    fn prune(&mut self, player: &Player) {
        for s in self.slots.iter_mut() {
            if s.is_some_and(|id| player.inventory.count(id) == 0) {
                *s = None;
            }
        }
    }
}

/// Shared state of the item panels and the hotbar: whether the inventory panel is open. While it
/// is, the panel owns the number keys and the wheel, to equip into hotbar slots.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct ItemUiState {
    pub inventory_visible: bool,
}

/// A shared handle to the [`HotbarState`]. Clones point at the same state.
#[derive(Clone, Default, Debug)]
pub struct HotbarHandle(Rc<Cell<HotbarState>>);

impl HotbarHandle {
    /// A fresh handle over an empty bar with the hand selected.
    pub fn new() -> Self {
        Self::default()
    }

    /// A copy of the current state.
    pub fn get(&self) -> HotbarState {
        self.0.get()
    }

    /// Replace the state.
    pub fn set(&self, state: HotbarState) {
        self.0.set(state);
    }

    /// Edit the state in place.
    pub fn update(&self, f: impl FnOnce(&mut HotbarState)) {
        let mut s = self.0.get();
        f(&mut s);
        self.0.set(s);
    }
}

/// A shared handle to the [`ItemUiState`]. Clones point at the same state.
#[derive(Clone, Default, Debug)]
pub struct ItemUiHandle(Rc<Cell<ItemUiState>>);

impl ItemUiHandle {
    /// A fresh handle: no item panel open.
    pub fn new() -> Self {
        Self::default()
    }

    /// A copy of the current state.
    pub fn get(&self) -> ItemUiState {
        self.0.get()
    }

    /// Replace the state.
    pub fn set(&self, state: ItemUiState) {
        self.0.set(state);
    }

    /// Whether the inventory panel is open (and owns the number keys and the wheel).
    pub fn inventory_visible(&self) -> bool {
        self.0.get().inventory_visible
    }

    /// Open or close the inventory panel.
    pub fn set_inventory_visible(&self, on: bool) {
        let mut s = self.0.get();
        s.inventory_visible = on;
        self.0.set(s);
    }
}

/// The package entry point: provides the shared [`ItemUiHandle`] and [`HotbarHandle`] to the
/// packages that depend on this one, and installs the hotbar mod over them.
pub fn register(registrar: &mut ModRegistrar) {
    let ui = ItemUiHandle::new();
    let bar = HotbarHandle::new();
    registrar.provide(ui.clone());
    registrar.provide(bar.clone());
    registrar.add(HotbarMod::new(ui, bar));
}

/// Slot edge and pitch on screen, in pixels.
const SLOT: i32 = 46;
const PITCH: i32 = 52;
const BOTTOM: i32 = -14;
const FRAME: Color = Color::new(10, 12, 16, 210);
const SELECTED: Color = Color::new(250, 214, 92, 255);

/// What the hotbar HUD shows: inventory revision, slots, screen size, registry size, tool-note generation.
type HudKey = (u64, HotbarState, i32, i32, u64, u64);

struct ToolNote {
    outcome: ToolUse,
    at: Instant,
    seq: u64,
}

/// The hotbar mod (id `hotbar`).
pub struct HotbarMod {
    ui: ItemUiHandle,
    bar: HotbarHandle,
    hud_cache: RefCell<Memo<HudKey, Vec<HudElement>>>,
    /// Last painted key, so a quiet frame does not format the tool line.
    hud_seen: Cell<Option<HudKey>>,
    note: Option<ToolNote>,
    note_seq: u64,
}

impl HotbarMod {
    /// A hotbar over shared handles (the same ones an inventory equips through).
    pub fn new(ui: ItemUiHandle, bar: HotbarHandle) -> Self {
        Self {
            ui,
            bar,
            hud_cache: RefCell::new(Memo::new()),
            hud_seen: Cell::new(None),
            note: None,
            note_seq: 0,
        }
    }

    fn live_note_gen(&self) -> u64 {
        match &self.note {
            Some(n) if n.at.elapsed().as_secs_f32() < 1.6 => n.seq,
            _ => 0,
        }
    }

    fn format_note(&self, world: &World) -> String {
        let Some(n) = &self.note else { return String::new() };
        if n.at.elapsed().as_secs_f32() >= 1.6 {
            return String::new();
        }
        let reg = world.registry();
        match n.outcome {
            ToolUse::NoReaction => "no reaction".to_string(),
            ToolUse::CellDissolved { cell } => format!("{} dissolved into the tool", reg.display_name(cell)),
            ToolUse::ToolDissolved { .. } => "the tool dissolved into the block".to_string(),
            ToolUse::Drew { cell } => format!("drew an element from {}", reg.display_name(cell)),
            ToolUse::Gave { cell } => format!("gave an element to {}", reg.display_name(cell)),
            ToolUse::Exchanged { cell } => format!("exchanged elements with {}", reg.display_name(cell)),
        }
    }

    fn with(&self, f: impl FnOnce(&mut HotbarState)) {
        self.bar.update(f);
    }
}

/// Pixel geometry of the bar at one screen size (designed at 720 px tall, scaled up from there).
struct Layout {
    slot: i32,
    pitch: i32,
    bottom: i32,
    k: f32,
}

impl Layout {
    fn of(screen_h: i32) -> Self {
        let k = (screen_h as f32 / 720.0).clamp(1.0, 4.0);
        let px = |v: i32| (v as f32 * k).round() as i32;
        Layout { slot: px(SLOT), pitch: px(PITCH), bottom: px(BOTTOM), k }
    }

    fn px(&self, v: i32) -> i32 {
        (v as f32 * self.k).round() as i32
    }

    /// Horizontal offset of slot `i` (0 = hand … 9) from the screen centre.
    fn slot_x(&self, i: usize) -> i32 {
        (2 * i as i32 - SLOTS as i32) * self.pitch / 2
    }
}

fn paint(state: &HotbarState, player: &Player, world: &World, screen_h: i32) -> Vec<HudElement> {
    let reg = world.registry();
    let l = Layout::of(screen_h);
    let (slot, bottom) = (l.slot, l.bottom);
    let mut out = Vec::with_capacity(5 * (SLOTS + 1) + 2);
    for i in 0..=SLOTS {
        let x = l.slot_x(i);
        if i == state.selected {
            let rim = l.px(3);
            out.push(HudElement::Rect { at: Anchor::Bottom, off: (x, bottom + rim), size: (slot + 2 * rim, slot + 2 * rim), color: SELECTED });
        }
        out.push(HudElement::Rect { at: Anchor::Bottom, off: (x, bottom), size: (slot, slot), color: FRAME });
        let key = i.to_string();
        out.push(HudElement::Label {
            at: Anchor::Bottom,
            off: (x - slot / 2 + l.px(7), bottom - slot + l.px(12)),
            base_fs: 12,
            role: Role::Dim,
            text: key.into(),
        });
        let id = if i == 0 { None } else { state.slots[i - 1] };
        match id {
            Some(id) => {
                // The material's colour with its accent: the HUD cannot draw the texture itself.
                let c = reg.color(id);
                let v = reg.visual(id);
                out.push(HudElement::Rect { at: Anchor::Bottom, off: (x, bottom - l.px(7)), size: (slot - l.px(12), slot - l.px(20)), color: c });
                out.push(HudElement::Rect {
                    at: Anchor::Bottom,
                    off: (x + l.px(6), bottom - l.px(10)),
                    size: (slot - l.px(26), l.px(6)),
                    color: Color::new(v.rgb2[0], v.rgb2[1], v.rgb2[2], 255),
                });
                let n = player.inventory.count(id);
                out.push(HudElement::Label {
                    at: Anchor::Bottom,
                    off: (x + slot / 2 - l.px(10), bottom - l.px(3)),
                    base_fs: 14,
                    role: Role::Primary,
                    text: n.to_string().into(),
                });
            }
            None if i == 0 => {
                out.push(HudElement::Label { at: Anchor::Bottom, off: (x, bottom - l.px(14)), base_fs: 12, role: Role::Muted, text: "hand".into() });
            }
            None => {}
        }
    }
    // What is held, by name: the naming mod's tool name, or "Bare hand".
    let title = match state.selected_id() {
        Some(id) => {
            let n = reg.configuration(id).len();
            format!("{}  ({} element{})", reg.tool_name(id), n, if n == 1 { "" } else { "s" })
        }
        None => "Bare hand".to_string(),
    };
    out.push(HudElement::Label { at: Anchor::Bottom, off: (0, bottom - slot - l.px(14)), base_fs: 18, role: Role::Warning, text: title.into() });
    out
}

impl Mod for HotbarMod {
    fn name(&self) -> &str {
        "Hotbar"
    }

    fn id(&self) -> &'static str {
        "hotbar"
    }

    fn description(&self) -> &str {
        "Nine slots of held materials: 1-9 / wheel to pick, left click uses it as a tool, right click places it. 0 = bare hand."
    }

    fn group(&self) -> &'static str {
        ESSENTIALS
    }

    fn actions(&self) -> &[Action] {
        ACTIONS
    }

    fn reset(&mut self) {
        self.bar.set(HotbarState::default());
        self.note = None;
    }

    fn update(&mut self, ctx: &mut ModContext) {
        let player = &*ctx.player;
        self.with(|s| s.prune(player));
        // While the inventory is open it owns the number keys and the wheel (to equip).
        if !self.ui.inventory_visible() {
            let mut key = None;
            if ctx.action(HAND_ID) {
                key = Some(0);
            }
            for (i, id) in SLOT_IDS.iter().enumerate() {
                if ctx.action(id) {
                    key = Some(i + 1);
                }
            }
            if let Some(k) = key {
                self.with(|s| {
                    s.selected = if k != 0 && s.selected == k { 0 } else { k };
                });
            }
            let cycle = i32::from(ctx.action(NEXT_ID)) - i32::from(ctx.action(PREV_ID));
            if cycle != 0 {
                self.with(|s| {
                    let n = SLOTS as i32 + 1;
                    s.selected = (s.selected as i32 + cycle).rem_euclid(n) as usize;
                });
            }
        }
        if ctx.place {
            let state = self.bar.get();
            let (Some(id), Some((x, y, z))) = (state.selected_id(), ctx.place_target) else { return };
            if ctx.world.block_at(x, y, z) != AIR {
                return;
            }
            // The core re-checks the cell and the player's body, refunding the unit if it refuses.
            if ctx.player.inventory.consume(id, 1) {
                ctx.placements.push((x, y, z, id));
            }
        }
    }

    fn on_block_break(&mut self, id: BlockId, _world: &World, overflow: bool) {
        if !overflow {
            self.with(|s| s.adopt(id));
        }
    }

    fn tool(&self, player: &Player) -> Option<BlockId> {
        self.bar.get().selected_id().filter(|&id| player.inventory.count(id) > 0)
    }

    fn on_tool_used(&mut self, outcome: ToolUse) {
        self.note_seq += 1;
        self.note = Some(ToolNote { outcome, at: Instant::now(), seq: self.note_seq });
    }

    fn on_tool_changed(&mut self, old: BlockId, new: BlockId) {
        self.with(|s| {
            if s.selected_id() == Some(old) {
                let k = s.selected;
                if new == AIR {
                    s.slots[k - 1] = None;
                } else {
                    s.equip(k, new);
                }
            }
        });
    }

    fn hud(&self, world: &World, player: &Player, screen: (i32, i32), out: &mut Vec<HudElement>) {
        let state = self.bar.get();
        let note_gen = self.live_note_gen();
        // Names arrive a frame after a configuration is interned: key on the table size too.
        let key = (player.inventory.rev(), state, screen.0, screen.1, world.registry().block_count() as u64, note_gen);
        if self.hud_seen.get() != Some(key) {
            let mut els = paint(&state, player, world, screen.1);
            if note_gen != 0 {
                let text = self.format_note(world);
                if !text.is_empty() {
                    els.push(HudElement::Label {
                        at: Anchor::Bottom,
                        off: (0, -112),
                        base_fs: 18,
                        role: Role::Muted,
                        text: text.into(),
                    });
                }
            }
            let _ = self.hud_cache.borrow_mut().get_or(key, || els);
            self.hud_seen.set(Some(key));
        }
        out.extend(self.hud_cache.borrow_mut().get_or(key, || paint(&state, player, world, screen.1)).iter().cloned());
    }

    fn save_state(&self, world: &World) -> Option<(u16, String)> {
        let s = self.bar.get();
        let mut text = format!("sel={}", s.selected);
        for (i, slot) in s.slots.iter().enumerate() {
            if let Some(id) = slot {
                text.push_str(&format!(";{}={}", i + 1, world.registry().spec(*id)));
            }
        }
        Some((1, text))
    }

    fn load_state(&mut self, _version: u16, data: &str, world: &mut World) -> u32 {
        let mut s = HotbarState::default();
        for part in data.split(';') {
            let Some((k, v)) = part.split_once('=') else { continue };
            if k == "sel" {
                s.selected = v.parse::<usize>().unwrap_or(0).min(SLOTS);
            } else if let Ok(key) = k.parse::<usize>()
                && let Some(id) = world.registry_mut().parse_spec(v)
            {
                s.equip(key, id);
            }
        }
        self.bar.set(s);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::engine::DVec3;
    use pwc_mod_api::material::{Configuration, Element};
    use pwc_mod_api::render_config::RenderConfig;
    use pwc_mod_api::world::generation::WorldgenKind;
    use pwc_mod_api::{GameBuild, ModDescriptor, Mods};

    fn setup() -> (HotbarMod, ItemUiHandle, World, Player, BlockId, BlockId) {
        let ui = ItemUiHandle::new();
        let m = HotbarMod::new(ui.clone(), HotbarHandle::new());
        let mut world = World::with_kind(5, RenderConfig::default(), WorldgenKind::Flat, true);
        let a = world.registry_mut().intern(&Configuration::single(Element::new([1, 2, 3, 4]))).unwrap();
        let b = world.registry_mut().intern(&Configuration::single(Element::new([9, 2, 3, 4]))).unwrap();
        let player = Player::new(DVec3::new(0.5, 70.0, 0.5));
        (m, ui, world, player, a, b)
    }

    fn ctx<'a>(player: &'a mut Player, world: &'a mut World) -> ModContext<'a> {
        ModContext::new(player, world)
    }

    const PACKAGE: ModDescriptor = ModDescriptor { id: "pwc.hotbar", name: "Hotbar", version: "2.0.0", register };

    /// This package alone, registered the way a PWC build registers it.
    fn build() -> Mods {
        GameBuild::new().with_mod(PACKAGE).mods()
    }

    #[test]
    fn gathered_materials_fill_empty_slots_and_keys_select_them() {
        let (mut m, _ui, mut world, mut player, a, b) = setup();
        player.inventory.add(a, 3);
        player.inventory.add(b, 1);
        m.on_block_break(a, &world, false);
        m.on_block_break(b, &world, false);
        m.on_block_break(a, &world, false);
        assert_eq!(m.bar.get().slots[..2], [Some(a), Some(b)]);
        assert_eq!(m.tool(&player), None, "the hand is selected until a key says otherwise");
        let mut c = ctx(&mut player, &mut world);
        c.set_action(SLOT_IDS[1]);
        m.update(&mut c);
        assert_eq!(m.tool(&player), Some(b));
        let mut c = ctx(&mut player, &mut world);
        c.set_action(SLOT_IDS[1]);
        m.update(&mut c);
        assert_eq!(m.tool(&player), None, "pressing the selected slot again returns to the hand");
        let mut c = ctx(&mut player, &mut world);
        c.set_action(PREV_ID);
        m.update(&mut c);
        assert_eq!(m.bar.get().selected, 9, "the wheel wraps from the hand to slot 9");
    }

    #[test]
    fn an_open_inventory_owns_the_number_keys_and_the_wheel() {
        let (mut m, ui, mut world, mut player, a, _) = setup();
        player.inventory.add(a, 1);
        m.on_block_break(a, &world, false);
        ui.set_inventory_visible(true);
        let mut c = ctx(&mut player, &mut world);
        c.set_action(SLOT_IDS[0]);
        c.set_action(NEXT_ID);
        m.update(&mut c);
        assert_eq!(m.bar.get().selected, 0, "keys and wheel go to the panel while it is open");
        ui.set_inventory_visible(false);
        let mut c = ctx(&mut player, &mut world);
        c.set_action(SLOT_IDS[0]);
        m.update(&mut c);
        assert_eq!(m.tool(&player), Some(a));
    }

    #[test]
    fn a_changed_tool_moves_its_slot_and_a_spent_one_clears_it() {
        let (mut m, _ui, _world, mut player, a, b) = setup();
        player.inventory.add(a, 1);
        m.with(|s| {
            s.equip(1, a);
            s.selected = 1;
        });
        m.on_tool_changed(a, b);
        assert_eq!(m.bar.get().selected_id(), Some(b));
        m.on_tool_changed(b, AIR);
        assert_eq!(m.bar.get().selected_id(), None);
    }

    #[test]
    fn placing_spends_one_unit_of_the_selected_slot() {
        let (mut m, _ui, mut world, mut player, a, _) = setup();
        player.inventory.add(a, 2);
        m.with(|s| {
            s.equip(3, a);
            s.selected = 3;
        });
        let mut c = ctx(&mut player, &mut world);
        c.place = true;
        c.place_target = Some((10, 64, 10));
        m.update(&mut c);
        assert_eq!(c.placements, vec![(10, 64, 10, a)]);
        assert_eq!(player.inventory.count(a), 1);
    }

    #[test]
    fn hotbar_state_round_trips_through_save_state() {
        let (mut m, _ui, mut world, _player, a, b) = setup();
        m.with(|s| {
            s.equip(1, a);
            s.equip(5, b);
            s.selected = 5;
        });
        let (v, text) = m.save_state(&world).unwrap();
        let saved = m.bar.get();
        m.reset();
        m.load_state(v, &text, &mut world);
        assert_eq!(m.bar.get(), saved);
    }

    #[test]
    fn register_provides_the_handles_the_mod_uses() {
        fn consumer(r: &mut ModRegistrar) {
            let ui = r.get::<ItemUiHandle>().expect("pwc.hotbar provides the item UI handle");
            let bar = r.get::<HotbarHandle>().expect("pwc.hotbar provides the hotbar handle");
            ui.set_inventory_visible(true);
            bar.update(|s| s.selected = 4);
        }
        let mods = GameBuild::new()
            .with_mod(PACKAGE)
            .with_mod(ModDescriptor { id: "test.consumer", name: "Consumer", version: "1.0.0", register: consumer })
            .mods();
        assert_eq!(mods.len(), 1);
        assert_eq!((mods.id(0), mods.name(0), mods.group(0)), ("hotbar", "Hotbar", ESSENTIALS));
        assert_eq!(mods.package(0), Some("pwc.hotbar"));
        assert!(mods.is_enabled(0), "the hotbar starts enabled");
        let world = World::new(1);
        let saved = mods.save_states(&world);
        assert_eq!(saved, [("hotbar".to_string(), "v1;sel=4".to_string())], "the consumer's handle is the mod's state");
    }

    #[test]
    fn save_states_key_by_id_and_load_accepts_display_name() {
        let mut world = World::new(1);
        let mut mods = build();
        let rock = world.registry().id_by_label("rock").unwrap();
        let spec = world.registry().spec(rock);
        mods.load_state("Hotbar", &format!("sel=2;2={spec}"), &mut world);
        let saved = mods.save_states(&world);
        assert!(saved.iter().any(|(k, _)| k == "hotbar"), "save keys are stable ids, not display names");
        assert!(!saved.iter().any(|(k, _)| k == "Hotbar"));
        let data = saved.iter().find(|(k, _)| k == "hotbar").map(|(_, d)| d.clone()).expect("hotbar persists");
        for key in ["hotbar", "Hotbar"] {
            let mut fresh = build();
            fresh.load_state(key, &data, &mut world);
            assert_eq!(
                fresh.save_states(&world).iter().find(|(k, _)| k == "hotbar").map(|(_, d)| d.as_str()),
                Some(data.as_str())
            );
        }
    }

    #[test]
    fn mod_state_round_trips_version_prefix() {
        let mut world = World::new(1);
        let mut mods = build();
        let rock = world.registry().id_by_label("rock").unwrap();
        let spec = world.registry().spec(rock);
        mods.load_state("hotbar", &format!("v1;sel=1;1={spec}"), &mut world);
        let saved = mods.save_states(&world);
        let bar = saved.iter().find(|(k, _)| k == "hotbar").map(|(_, d)| d.as_str()).expect("hotbar");
        assert_eq!(bar, format!("v1;sel=1;1={spec}"));
        let mut fresh = build();
        for (k, v) in &saved {
            fresh.load_state(k, v, &mut world);
        }
        assert_eq!(fresh.save_states(&world), saved);
    }

    #[test]
    fn an_old_save_loads_the_same_slots_and_selection() {
        let mut world = World::new(1);
        let mut mods = build();
        let a = world.registry_mut().intern(&Configuration::single(Element::new([1, 2, 3, 4]))).unwrap();
        let b = world.registry_mut().intern(&Configuration::single(Element::new([9, 2, 3, 4]))).unwrap();
        let spec_a = world.registry().spec(a);
        let spec_b = world.registry().spec(b);
        mods.load_state("hotbar", &format!("v1;sel=3;1={spec_a};3={spec_b}"), &mut world);
        let saved = mods.save_states(&world);
        let line = saved.iter().find(|(k, _)| k == "hotbar").map(|(_, d)| d.clone()).expect("hotbar");
        assert_eq!(line, format!("v1;sel=3;1={spec_a};3={spec_b}"));
        let mut player = Player::new(DVec3::new(0.5, 70.0, 0.5));
        player.inventory.add(a, 1);
        player.inventory.add(b, 1);
        assert_eq!(mods.tool(&player), Some(b), "selection 3 is the second saved slot");
    }

    #[test]
    fn a_tool_note_is_a_label_above_the_bar() {
        let (mut m, _ui, world, player, _, _) = setup();
        m.on_tool_used(ToolUse::NoReaction);
        let mut out = Vec::new();
        m.hud(&world, &player, (1280, 720), &mut out);
        assert!(out.iter().any(|el| matches!(
            el,
            HudElement::Label { text, at: Anchor::Bottom, off: (0, -112), base_fs: 18, role: Role::Muted, .. } if &**text == "no reaction"
        )));
    }

    #[test]
    fn choices_persist_only_the_on_off_line() {
        let mut mods = build();
        assert_eq!(mods.choices_text(), "version=2\nhotbar=on\n", "the hotbar has no knobs");
        mods.set_group_enabled(ESSENTIALS, false);
        let text = mods.choices_text();
        assert!(text.contains("hotbar=off"), "{text}");
        let mut fresh = build();
        fresh.apply_choices_text(&text);
        assert!(!fresh.is_enabled(0));
    }
}
