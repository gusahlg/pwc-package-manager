//! The default inventory mod: the core inventory made visible, and the way to equip what you hold.
//!
//! The core inventory is a list of configurations. Counts of one configuration add together.
//! Press I to open this panel: ↑/↓ (or the wheel) choose a material, a number key 1-9 equips it
//! into that hotbar slot, Enter equips it into the slot currently selected (or the first free one
//! when the hand is selected), Esc or I closes. The counts live on the player
//! ([`pwc_mod_api::inventory::Inventory`]); switching the mod off makes the list inaccessible
//! again but keeps every unit on the player.
//!
//! The panel equips into the hotbar through the handles the `pwc.hotbar` package provides
//! ([`pwc_hotbar::HotbarHandle`], [`pwc_hotbar::ItemUiHandle`]).
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use pwc_hotbar::{HotbarHandle, HotbarState, ItemUiHandle, SLOT_IDS};
use pwc_mod_api::block::BlockId;
use pwc_mod_api::derived::Memo;
use pwc_mod_api::engine::Key;
use pwc_mod_api::input::intent::Chord;
use pwc_mod_api::inventory::Inventory;
use pwc_mod_api::player::Player;
use pwc_mod_api::ui::{visible_window, Anchor, HudElement, Panel, Role, Row, PANEL_FONT};
use pwc_mod_api::world::World;
use pwc_mod_api::{Action, HudFacts, Mod, ModContext, ModRegistrar, ESSENTIALS};

const TOGGLE: &[Chord] = &[Chord::key(Key::I)];
const ACTIONS: &[Action] = &[Action {
    id: "inventory.toggle",
    label: "Inventory",
    default: TOGGLE,
    repeat: false,
    held: false,
    immediate: false,
}];

/// How long the "elements lost" warning stays on screen after the last overflowing break.
const OVERFLOW_WARNING: Duration = Duration::from_millis(2500);

/// Left edge of the panel, in pixels.
pub const PANEL_X: i32 = 12;
/// Top edge of the panel, in pixels.
pub const PANEL_Y: i32 = 44;
const PANEL_WIDTH: i32 = 420;
const PANEL_PAD: i32 = 8;
const FONT_SIZE: i32 = 18;
const LINE_HEIGHT: i32 = FONT_SIZE + 4;
/// The chat scrollback and the hotbar occupy the bottom of the screen.
const BOTTOM_RESERVE: i32 = 260;

/// The package entry point: installs the inventory over the hotbar's shared handles. `pwc.hotbar`
/// is a dependency, so it has always registered (and provided the handles) before this runs.
pub fn register(registrar: &mut ModRegistrar) {
    let ui = registrar
        .get::<ItemUiHandle>()
        .expect("pwc.inventory needs the item UI handle from pwc.hotbar (a declared dependency registered first)");
    let bar = registrar
        .get::<HotbarHandle>()
        .expect("pwc.inventory needs the hotbar handle from pwc.hotbar (a declared dependency registered first)");
    registrar.add(InventoryMod::new(ui, bar));
}

/// The "elements lost" notice: raised by a break the inventory could not hold, it lapses
/// [`OVERFLOW_WARNING`] after the most recent such break.
#[derive(Default)]
struct LossNotice {
    raised: Option<Instant>,
}

impl LossNotice {
    fn raise(&mut self) {
        self.raised = Some(Instant::now());
    }

    fn showing(&self) -> bool {
        self.raised.is_some_and(|at| at.elapsed() <= OVERFLOW_WARNING)
    }

    fn clear(&mut self) {
        self.raised = None;
    }
}

/// The inventory list over the core inventory, with a cursor that equips into the hotbar.
pub struct InventoryMod {
    ui: ItemUiHandle,
    bar: HotbarHandle,
    /// Row under the cursor (index into the inventory's first-seen order).
    cursor: Cell<usize>,
    /// Shown while a recent break overflowed the inventory (units were destroyed).
    loss: LossNotice,
    /// Formatted HUD, rebuilt only when what it shows changes.
    hud_cache: RefCell<Memo<HudKey, Vec<HudElement>>>,
}

/// What the panel shows: inventory revision, screen size, visibility, overflow warning, cursor, slots,
/// registry size.
type HudKey = (u64, i32, i32, bool, bool, usize, HotbarState, usize);

impl InventoryMod {
    /// An inventory over the hotbar's shared handles.
    pub fn new(ui: ItemUiHandle, bar: HotbarHandle) -> Self {
        Self { ui, bar, cursor: Cell::new(0), loss: LossNotice::default(), hud_cache: RefCell::new(Memo::new()) }
    }

    fn visible(&self) -> bool {
        self.ui.inventory_visible()
    }

    fn set_visible(&self, on: bool) {
        self.ui.set_inventory_visible(on);
    }

    /// Equip the material under the cursor into hotbar slot `key` (1..=9).
    fn equip(&self, inventory: &Inventory, key: usize) {
        let Some((id, _)) = inventory.iter().nth(self.cursor.get()) else { return };
        let mut bar = self.bar.get();
        bar.equip(key, id);
        bar.selected = key;
        self.bar.set(bar);
    }
}

fn row_capacity(screen_h: i32) -> usize {
    let content_y = PANEL_Y + PANEL_PAD + 2 * LINE_HEIGHT + 2;
    ((screen_h - BOTTOM_RESERVE - content_y) / LINE_HEIGHT).max(1) as usize
}

#[allow(clippy::too_many_arguments)]
fn paint_inventory(
    inventory: &Inventory,
    world: &World,
    bar: &HotbarState,
    cursor: usize,
    screen_w: i32,
    screen_h: i32,
    visible: bool,
    overflow: bool,
) -> Vec<HudElement> {
    if !visible {
        if overflow {
            // The panel is shut, so the warning takes the header's place on its own line.
            return vec![HudElement::Label {
                at: Anchor::Top,
                off: (0, PANEL_Y),
                base_fs: PANEL_FONT,
                role: Role::Danger,
                text: "Inventory full - elements lost!".into(),
            }];
        }
        return Vec::new();
    }
    let width = PANEL_WIDTH.min((screen_w - PANEL_X * 2).max(1));
    let total = inventory.total();
    let items: Vec<(BlockId, u32)> = inventory.iter().collect();
    let header = vec![
        if overflow {
            Row::new(Role::Danger, "Inventory full - elements lost!")
        } else {
            Row::new(Role::Warning, format!("Inventory · {total}/{} held", inventory.capacity()))
        },
        Row::new(Role::Dim, "↑↓ choose · 1-9 equip · Enter: current slot · I close"),
    ];
    let mut rows = Vec::new();
    if items.is_empty() {
        rows.push(Row::new(Role::Muted, "(empty — break something with the bare hand)"));
    } else {
        let reg = world.registry();
        for i in visible_window(items.len(), cursor, row_capacity(screen_h)) {
            let (id, count) = items[i];
            let slot = bar.key_of(id).map_or(String::new(), |k| format!("[{k}] "));
            let marker = if i == cursor { "▶ " } else { "  " };
            let n = reg.configuration(id).len();
            let role = if i == cursor { Role::Accent } else { Role::Muted };
            rows.push(
                Row::new(role, format!("{marker}{slot}{count}x {}  · {n} el.", reg.display_name(id)))
                    .with_swatch(reg.color(id)),
            );
        }
    }
    vec![HudElement::Panel(Panel { at: (PANEL_X, PANEL_Y), width, header: header.into(), rows: rows.into() })]
}

impl Mod for InventoryMod {
    fn name(&self) -> &str {
        "Inventory"
    }

    fn id(&self) -> &'static str {
        "inventory"
    }

    fn description(&self) -> &str {
        "Your held materials (press I): choose one and press 1-9 to equip it on the hotbar."
    }

    fn actions(&self) -> &[Action] {
        ACTIONS
    }

    fn group(&self) -> &'static str {
        ESSENTIALS
    }

    fn update(&mut self, ctx: &mut ModContext) {
        // The inventory key flips the panel; a lapsed loss notice is dropped every tick.
        let toggle = ctx.mod_ui && ctx.action("inventory.toggle");
        let open = self.visible() != toggle;
        self.set_visible(open);
        if !self.loss.showing() {
            self.loss.clear();
        }
        if !open {
            return;
        }
        let kinds = ctx.player.inventory.iter().count();
        let mut cursor = self.cursor.get().min(kinds.saturating_sub(1));
        if ctx.nav_up {
            cursor = cursor.saturating_sub(1);
        }
        if ctx.nav_down && cursor + 1 < kinds {
            cursor += 1;
        }
        self.cursor.set(cursor);
        let mut key = None;
        for (i, id) in SLOT_IDS.iter().enumerate() {
            if ctx.action(id) {
                key = Some(i + 1);
            }
        }
        if let Some(k) = key {
            self.equip(&ctx.player.inventory, k);
        }
        if ctx.nav_confirm {
            let bar = self.bar.get();
            let key = if bar.selected != 0 {
                bar.selected
            } else {
                bar.slots.iter().position(|s| s.is_none()).map_or(1, |i| i + 1)
            };
            self.equip(&ctx.player.inventory, key);
        }
    }

    fn reset(&mut self) {
        self.set_visible(false);
        self.cursor.set(0);
        self.loss.clear();
    }

    fn close_overlay(&mut self) -> bool {
        if self.visible() {
            self.set_visible(false);
            return true;
        }
        false
    }

    fn on_block_break(&mut self, _id: BlockId, _world: &World, overflow: bool) {
        if overflow {
            self.loss.raise();
        }
    }

    fn hud(&self, facts: &HudFacts, world: &World, player: &Player, out: &mut Vec<HudElement>) {
        // Gameplay UI: every HUD mode but Off.
        if !facts.hud_mode.shows_mod_hud() {
            return;
        }
        let (screen_w, screen_h) = facts.screen;
        let overflow = self.loss.showing();
        let visible = self.visible();
        let bar = self.bar.get();
        let cursor = self.cursor.get();
        let key = (
            player.inventory.rev(),
            screen_w,
            screen_h,
            visible,
            overflow,
            cursor,
            bar,
            world.registry().block_count(),
        );
        let inventory = &player.inventory;
        let mut cache = self.hud_cache.borrow_mut();
        let cached = cache.get_or(key, || paint_inventory(inventory, world, &bar, cursor, screen_w, screen_h, visible, overflow));
        out.extend(cached.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::engine::DVec3;
    use pwc_mod_api::render_config::RenderConfig;
    use pwc_mod_api::world::generation::WorldgenKind;
    use pwc_mod_api::{GameBuild, ModDescriptor, Mods};

    fn world() -> World {
        World::with_kind(1, RenderConfig::default(), WorldgenKind::Flat, true)
    }

    fn inventory() -> InventoryMod {
        InventoryMod::new(ItemUiHandle::new(), HotbarHandle::new())
    }

    /// `pwc.hotbar` and this package, registered the way a PWC build registers them.
    fn build() -> Mods {
        GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.hotbar", name: "Hotbar", version: "2.0.0", register: pwc_hotbar::register })
            .with_mod(ModDescriptor { id: "pwc.inventory", name: "Inventory", version: "2.0.0", register })
            .mods()
    }

    fn ctx<'a>(player: &'a mut Player, world: &'a mut World) -> ModContext<'a> {
        ModContext::new(player, world)
    }

    /// Flatten HUD labels and panel rows to text.
    fn hud_text(elements: &[HudElement]) -> String {
        let mut out = String::new();
        for el in elements {
            match el {
                HudElement::Label { text, .. } => {
                    out.push_str(text);
                    out.push('\n');
                }
                HudElement::Panel(panel) => {
                    for row in panel.header.iter().chain(panel.rows.iter()) {
                        out.push_str(&row.text);
                        out.push('\n');
                    }
                }
                HudElement::Rect { .. } => {}
            }
        }
        out
    }

    #[test]
    fn overflowing_break_notification_arms_the_warning() {
        let world = world();
        let mut inventory = inventory();
        let rock = world.registry().id_by_label("rock").unwrap();
        inventory.on_block_break(rock, &world, false);
        assert!(inventory.loss.raised.is_none(), "no warning while everything fits");
        inventory.on_block_break(rock, &world, true);
        let armed = inventory.loss.raised.expect("dropping units must arm the warning");
        assert!(armed.elapsed() <= OVERFLOW_WARNING);
        assert!(inventory.loss.showing());
        let mut shown = Vec::new();
        inventory.hud(&HudFacts::new((800, 600)), &world, &Player::new(DVec3::new(0.0, 70.0, 0.0)), &mut shown);
        assert_eq!(hud_text(&shown), "Inventory full - elements lost!\n", "the closed panel still warns");
        let mut off = HudFacts::new((800, 600));
        off.hud_mode = pwc_mod_api::ui::HudMode::Off;
        let mut hidden = Vec::new();
        inventory.hud(&off, &world, &Player::new(DVec3::new(0.0, 70.0, 0.0)), &mut hidden);
        assert!(hidden.is_empty(), "the HUD off hides the warning too");
        inventory.reset();
        assert!(inventory.loss.raised.is_none(), "reset must clear the warning");
    }

    #[test]
    fn a_lapsed_warning_is_dropped_on_update() {
        let mut world = world();
        let mut player = Player::new(DVec3::new(0.0, 70.0, 0.0));
        let mut inv = inventory();
        inv.loss.raised = Some(Instant::now() - OVERFLOW_WARNING - Duration::from_millis(1));
        assert!(!inv.loss.showing());
        inv.update(&mut ctx(&mut player, &mut world));
        assert!(inv.loss.raised.is_none());
        inv.on_block_break(world.registry().id_by_label("rock").unwrap(), &world, true);
        inv.update(&mut ctx(&mut player, &mut world));
        assert!(inv.loss.raised.is_some(), "a fresh warning survives the tick");
    }

    #[test]
    fn the_inventory_key_toggles_the_panel() {
        let mut world = world();
        let mut player = Player::new(DVec3::new(0.0, 70.0, 0.0));
        let inv_ui = ItemUiHandle::new();
        let mut inv = InventoryMod::new(inv_ui.clone(), HotbarHandle::new());
        let mut c = ctx(&mut player, &mut world);
        c.set_action("inventory.toggle");
        inv.update(&mut c);
        assert!(inv_ui.inventory_visible(), "I opens the panel, and the shared handle says so");
        inv.update(&mut ctx(&mut player, &mut world));
        assert!(inv_ui.inventory_visible(), "no key, no change");
        let mut c = ctx(&mut player, &mut world);
        c.set_action("inventory.toggle");
        inv.update(&mut c);
        assert!(!inv_ui.inventory_visible(), "I again closes it");
        assert!(!inv.close_overlay(), "nothing to close");
    }

    #[test]
    fn number_keys_equip_the_row_under_the_cursor() {
        let mut world = world();
        let mut player = Player::new(DVec3::new(0.0, 70.0, 0.0));
        let rock = world.registry().id_by_label("rock").unwrap();
        let soil = world.registry().id_by_label("soil").unwrap();
        player.inventory.add(rock, 2);
        player.inventory.add(soil, 1);
        let mut inv = inventory();
        inv.set_visible(true);
        let mut c = ctx(&mut player, &mut world);
        c.nav_down = true;
        c.set_action(SLOT_IDS[3]);
        inv.update(&mut c);
        let bar = inv.bar.get();
        assert_eq!(bar.slots[3], Some(soil), "cursor moved to the second row, key 4 equipped it");
        assert_eq!(bar.selected, 4, "equipping selects the slot");
        assert!(inv.close_overlay(), "Esc closes the open panel");
        assert!(!inv.close_overlay());
    }

    #[test]
    fn enter_equips_into_the_selected_or_first_free_slot() {
        let mut world = world();
        let mut player = Player::new(DVec3::new(0.0, 70.0, 0.0));
        let rock = world.registry().id_by_label("rock").unwrap();
        player.inventory.add(rock, 1);
        let mut inv = inventory();
        inv.set_visible(true);
        inv.bar.update(|s| s.slots[0] = Some(world.registry().id_by_label("soil").unwrap()));
        let mut c = ctx(&mut player, &mut world);
        c.nav_confirm = true;
        inv.update(&mut c);
        assert_eq!(inv.bar.get().slots[1], Some(rock), "with the hand selected, the first free slot");
        assert_eq!(inv.bar.get().selected, 2);
        inv.bar.update(|s| s.selected = 7);
        let mut c = ctx(&mut player, &mut world);
        c.nav_confirm = true;
        inv.update(&mut c);
        let bar = inv.bar.get();
        assert_eq!((bar.slots[1], bar.slots[6], bar.selected), (None, Some(rock), 7), "the selected slot");
    }

    #[test]
    fn register_shares_the_hotbar_handles() {
        let mut world = world();
        let mut player = Player::new(DVec3::new(0.0, 70.0, 0.0));
        let rock = world.registry().id_by_label("rock").unwrap();
        player.inventory.add(rock, 1);
        let mut mods = build();
        let ids: Vec<&str> = (0..mods.len()).map(|i| mods.id(i)).collect();
        assert_eq!(ids, ["hotbar", "inventory"], "dependency order: the hotbar registers first");
        assert_eq!(mods.package(1), Some("pwc.inventory"));
        assert_eq!(mods.group(1), ESSENTIALS);
        let mut c = ctx(&mut player, &mut world);
        c.set_action("inventory.toggle");
        mods.update(&mut c);
        let mut c = ctx(&mut player, &mut world);
        c.set_action(SLOT_IDS[2]);
        mods.update(&mut c);
        assert_eq!(mods.tool(&player), Some(rock), "the inventory equipped into the hotbar's own state");
        assert!(mods.close_overlay(), "Esc closes the inventory first");
        let mut c = ctx(&mut player, &mut world);
        c.set_action(SLOT_IDS[2]);
        mods.update(&mut c);
        assert_eq!(mods.tool(&player), None, "once closed, the number keys belong to the hotbar again");
    }

    #[test]
    #[should_panic(expected = "pwc.inventory needs the item UI handle from pwc.hotbar")]
    fn register_without_the_hotbar_names_the_missing_dependency() {
        let _ = GameBuild::new()
            .with_mod(ModDescriptor { id: "pwc.inventory", name: "Inventory", version: "2.0.0", register })
            .mods();
    }

    #[test]
    fn the_inventory_is_core_state_not_an_inventory_save_line() {
        let mut world = World::new(1);
        let mut mods = build();
        let rock = world.registry().id_by_label("rock").unwrap();
        let spec = world.registry().spec(rock);
        mods.load_state("hotbar", &format!("v1;sel=1;1={spec}"), &mut world);
        let saved = mods.save_states(&world);
        assert!(saved.iter().all(|(k, _)| k != "inventory"), "the inventory is core state, not an inventory save line");
        assert!(saved.iter().any(|(k, _)| k == "hotbar"));
        let text = mods.choices_text();
        assert!(text.contains("inventory=on") && !text.contains("inventory.state"), "{text}");
        mods.apply_choices_text("version=2\ninventory=off\nnot-a-mod=on\n");
        assert!(!mods.is_enabled(1));
        assert!(mods.is_enabled(0), "an unmentioned mod keeps its default (on)");
    }

    #[test]
    fn disabling_inventory_does_not_destroy_mined_blocks() {
        let world = world();
        let mut player = Player::new(DVec3::new(0.0, 70.0, 0.0));
        let mut mods = build();
        let rock = world.registry().id_by_label("rock").unwrap();
        assert!(player.inventory.add(rock, 2));
        mods.set_enabled("inventory", false);
        mods.on_block_break(rock, &world, false);
        assert_eq!(player.inventory.count(rock), 2, "core keeps the configurations");
        mods.set_enabled("inventory", true);
        let inv = inventory();
        inv.set_visible(true);
        let mut shown = Vec::new();
        inv.hud(&HudFacts::new((800, 600)), &world, &player, &mut shown);
        let text = hud_text(&shown);
        assert!(text.contains(&format!("2x {}", world.registry().display_name(rock))), "{text}");
        assert!(!text.contains("rock"), "labels never reach the player: {text}");
    }
}
