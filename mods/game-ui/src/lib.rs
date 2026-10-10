//! Game UI: the in-world HUD of the essentials. Two pieces:
//!
//! - **The information HUD** ([`info`]): the reticle, the loading line, the coordinates, the frame
//!   rate, the player count and the "connection interrupted" banner, drawn from the facts the core
//!   passes to `hud`. The core draws no HUD text of its own.
//! - **The facing indicator**: which way the player looks along the world's own (physical) X, Y
//!   and Z axes.
//!
//! Under the minimap, in the top-right corner, a small gizmo shows where +X, +Y and +Z point on
//! screen (an axis pointing into the screen is drawn dimmer, nearer axes on top), and under it the
//! facing in words, the dominant signed axis plus the next one when the view leans toward it by
//! more than 22.5° (`facing -Z +Y`), then the view direction as a vector.
//!
//! The HUD can draw labels, panels and filled rectangles but no lines, so each gizmo axis is a
//! row of small squares. The elements are rebuilt only when what they show changes: the
//! direction to the hundredth, the gizmo to the pixel, or the screen size.

use std::cell::RefCell;

use pwc_mod_api::derived::Memo;
use pwc_mod_api::engine::Color;
use pwc_mod_api::player::Player;
use pwc_mod_api::ui::{Anchor, HudElement, Role};
use pwc_mod_api::world::World;
use pwc_mod_api::{HudFacts, Mod, ModRegistrar};

mod info;

/// The package entry point: installs the Game UI mod.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(GameUi::default());
}

/// The Game UI mod (id `game_ui`).
pub struct GameUi {
    info: info::Info,
    hud: RefCell<Memo<Facing, Vec<HudElement>>>,
}

// By hand: `Memo`'s derived `Default` would require one of `Facing` too.
impl Default for GameUi {
    fn default() -> Self {
        Self { info: info::Info::default(), hud: RefCell::new(Memo::new()) }
    }
}

impl Mod for GameUi {
    fn name(&self) -> &str {
        "Game UI"
    }

    fn id(&self) -> &'static str {
        "game_ui"
    }

    fn hud(&self, facts: &HudFacts, _world: &World, player: &Player, out: &mut Vec<HudElement>) {
        self.info.hud(facts, player, out);
        // The facing indicator is gameplay UI: every HUD mode but Off.
        if facts.hud_mode.shows_mod_hud() {
            let facing = Facing::of(player, facts.screen);
            out.extend(self.hud.borrow_mut().get_or(facing, || facing.paint()).iter().cloned());
        }
    }
}

/// The axis names and their colours: X red, Y green, Z blue (as close as the UI palette gets).
const AXES: [(&str, Role); 3] = [("X", Role::Danger), ("Y", Role::Positive), ("Z", Role::Accent)];
/// The secondary axis is named once the view leans toward it by more than 22.5° (sin = 0.383).
const LEAN_CENTS: i16 = 38;
const BACKDROP: Color = Color::new(8, 10, 14, 150);
/// The minimap's right margin, shared so the two line up.
const MARGIN: i32 = 12;
/// Below the minimap and the multiplayer player count (its label reaches y = 220 at UI scale 2).
const TOP: i32 = 232;
/// Room for one line of `base_fs` text at the largest UI scale (2.0), plus a small gap.
const fn line_room(base_fs: i32) -> i32 {
    2 * base_fs + 4
}

/// Everything the indicator shows, quantised to what it can display: the cache key.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Facing {
    screen: (i32, i32),
    /// The view direction in hundredths, per world axis.
    cents: [i16; 3],
    /// Where each axis tip sits on screen, in pixels from the gizmo's centre (y down).
    tips: [(i16, i16); 3],
}

/// The gizmo's geometry at one screen size (designed at 720 px tall, scaled up from there, like
/// the hotbar's).
struct Layout {
    k: f64,
}

impl Layout {
    fn of(screen_h: i32) -> Self {
        Layout { k: (screen_h as f64 / 720.0).clamp(1.0, 4.0) }
    }

    fn px(&self, v: f64) -> i32 {
        (v * self.k).round() as i32
    }

    /// Half the backdrop's edge.
    fn half(&self) -> i32 {
        self.px(40.0)
    }

    /// The gizmo's centre, absolute pixels.
    fn centre(&self, screen_w: i32) -> (i32, i32) {
        (screen_w - MARGIN - self.half(), TOP + self.half())
    }

    /// Axis length on screen.
    fn reach(&self) -> f64 {
        30.0 * self.k
    }
}

impl Facing {
    fn of(player: &Player, screen: (i32, i32)) -> Self {
        let forward = player.forward();
        // The camera's right and up (a right-handed look-at along the body's up).
        let right = forward.cross(player.up()).normalize_or(forward.any_orthonormal_vector());
        let up = right.cross(forward);
        let reach = Layout::of(screen.1).reach();
        let cents = forward.to_array().map(|c| (c * 100.0).round() as i16);
        let mut tips = [(0, 0); 3];
        for (i, tip) in tips.iter_mut().enumerate() {
            *tip = ((right[i] * reach).round() as i16, (-up[i] * reach).round() as i16);
        }
        Self { screen, cents, tips }
    }

    /// Axis indices from the one pointing farthest into the screen (the view direction's largest
    /// component) to the nearest: the drawing order.
    fn depth_order(&self) -> [usize; 3] {
        let mut order = [0, 1, 2];
        order.sort_by_key(|&i| std::cmp::Reverse(self.cents[i]));
        order
    }

    /// `facing +X`, or `facing -Z +Y` while the view leans toward a second axis.
    fn words(&self) -> String {
        let mut by_size = [0, 1, 2];
        by_size.sort_by_key(|&i| std::cmp::Reverse(self.cents[i].abs()));
        let named = |i: usize| format!("{}{}", if self.cents[i] < 0 { '-' } else { '+' }, AXES[i].0);
        let (first, second) = (by_size[0], by_size[1]);
        if self.cents[second].abs() >= LEAN_CENTS {
            format!("facing {} {}", named(first), named(second))
        } else {
            format!("facing {}", named(first))
        }
    }

    /// The direction as `(+0.71, +0.71, +0.00)`.
    fn vector(&self) -> String {
        let [x, y, z] = self.cents.map(|c| {
            let a = c.unsigned_abs();
            format!("{}{}.{:02}", if c < 0 { '-' } else { '+' }, a / 100, a % 100)
        });
        format!("({x}, {y}, {z})")
    }

    fn paint(&self) -> Vec<HudElement> {
        let l = Layout::of(self.screen.1);
        // Gizmo elements are centred on an absolute point, whatever their size or the UI scale.
        let at = |(x, y): (i32, i32)| (x - self.screen.0 / 2, y - self.screen.1 / 2);
        let ((cx, cy), half) = (l.centre(self.screen.0), l.half());
        let mut out = vec![HudElement::Rect { at: Anchor::Center, off: at((cx, cy)), size: (2 * half, 2 * half), color: BACKDROP }];
        let (dot, step) = (l.px(3.0).max(2), l.px(2.0).max(1) as f64);
        for i in self.depth_order() {
            let (name, role) = AXES[i];
            let base = role.color();
            // An axis pointing into the screen is dimmer.
            let color = Color::new(base.r, base.g, base.b, if self.cents[i] > 0 { 140 } else { 255 });
            let (tx, ty) = (self.tips[i].0 as f64, self.tips[i].1 as f64);
            let dots = (tx.hypot(ty) / step).ceil().max(1.0) as i32;
            for d in 1..=dots {
                let t = d as f64 / dots as f64;
                let p = (cx + (tx * t).round() as i32, cy + (ty * t).round() as i32);
                out.push(HudElement::Rect { at: Anchor::Center, off: at(p), size: (dot, dot), color });
            }
            let label = (cx + (tx * 1.3).round() as i32, cy + (ty * 1.3).round() as i32);
            out.push(HudElement::Label { at: Anchor::Center, off: at(label), base_fs: 12, role, text: name.into() });
        }
        // The words line up with the gizmo's right edge, so a long line grows to the left.
        // Line gaps leave room for the text at any UI scale.
        let below = TOP + 2 * half + l.px(8.0);
        out.push(HudElement::Label { at: Anchor::TopRight, off: (-MARGIN, below), base_fs: 16, role: Role::Primary, text: self.words().into() });
        out.push(HudElement::Label { at: Anchor::TopRight, off: (-MARGIN, below + line_room(16)), base_fs: 13, role: Role::Muted, text: self.vector().into() });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::engine::DVec3;
    use pwc_mod_api::ui::HudMode;
    use pwc_mod_api::{GameBuild, ModDescriptor};

    const SCREEN: (i32, i32) = (1280, 720);

    /// Loaded single-player facts at `mode`, with no frame-rate reading yet.
    fn facts(mode: HudMode) -> HudFacts {
        let mut facts = HudFacts::new(SCREEN);
        facts.hud_mode = mode;
        facts
    }


    /// A player at the origin looking `yaw`, `pitch` degrees in the identity (Y-up) frame.
    fn looking(yaw: f32, pitch: f32) -> Player {
        let mut p = Player::new(DVec3::ZERO);
        p.orientation.yaw = yaw.to_radians();
        p.orientation.pitch = pitch.to_radians();
        p
    }

    fn labels(elements: &[HudElement]) -> Vec<&str> {
        elements
            .iter()
            .filter_map(|e| match e {
                HudElement::Label { text, .. } => Some(&**text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn names_the_dominant_axis_and_a_strong_lean() {
        let f = Facing::of(&looking(0.0, 0.0), SCREEN);
        assert_eq!((f.words().as_str(), f.vector().as_str()), ("facing +X", "(+1.00, +0.00, +0.00)"));
        // Yaw -90° faces -Z; 30° up leans well toward +Y.
        let f = Facing::of(&looking(-90.0, 30.0), SCREEN);
        assert_eq!((f.words().as_str(), f.vector().as_str()), ("facing -Z +Y", "(+0.00, +0.50, -0.87)"));
        // A mild tilt keeps one axis.
        assert_eq!(Facing::of(&looking(-90.0, 10.0), SCREEN).words(), "facing -Z");
        assert_eq!(Facing::of(&looking(180.0, -60.0), SCREEN).words(), "facing -Y -X");
    }

    /// Facing +X with +Y up: +Z points right, +Y up, +X straight into the screen.
    #[test]
    fn the_gizmo_shows_where_each_axis_points_on_screen() {
        let f = Facing::of(&looking(0.0, 0.0), SCREEN);
        let reach = Layout::of(SCREEN.1).reach().round() as i16;
        assert_eq!(f.tips, [(0, 0), (0, -reach), (reach, 0)]);
        assert_eq!(f.depth_order()[0], 0, "the axis pointing away draws first, under the others");
        // Facing -Z: +X points right.
        let f = Facing::of(&looking(-90.0, 0.0), SCREEN);
        assert_eq!(f.tips[0], (reach, 0));
    }

    #[test]
    fn the_hud_draws_the_gizmo_and_the_words() {
        let ui = GameUi::default();
        let (world, mut out) = (World::new(1), Vec::new());
        // Minimal: the reticle, then the indicator (the readouts are Full only).
        ui.hud(&facts(HudMode::Minimal), &world, &looking(-90.0, 30.0), &mut out);
        // Back to front: +Y leans into the screen, +Z points at the viewer.
        assert_eq!(labels(&out), ["Y", "X", "Z", "facing -Z +Y", "(+0.00, +0.50, -0.87)"]);
        let rects = out.iter().filter(|e| matches!(e, HudElement::Rect { .. })).count();
        assert!(rects > 20, "a backdrop and a row of dots per axis: {rects}");
    }

    /// An unchanged view reuses the cached elements (the same shared strings, nothing rebuilt);
    /// turning rebuilds them.
    #[test]
    fn a_still_view_reuses_its_elements() {
        let ui = GameUi::default();
        let world = World::new(1);
        let text = |player: &Player| {
            let mut out = Vec::new();
            ui.hud(&facts(HudMode::Full), &world, player, &mut out);
            match out.last() {
                Some(HudElement::Label { text, .. }) => text.clone(),
                _ => panic!("the vector label comes last"),
            }
        };
        let still = looking(10.0, 5.0);
        let (a, b) = (text(&still), text(&still));
        assert!(std::sync::Arc::ptr_eq(&a, &b));
        let turned = looking(40.0, 5.0);
        assert!(!std::sync::Arc::ptr_eq(&a, &text(&turned)));
    }

    /// Full mode: the reticle, the coordinates, the frame rate and (on a server) the player count;
    /// Minimal keeps the reticle and the facing indicator; Off shows nothing.
    #[test]
    fn each_hud_mode_shows_its_pieces() {
        let ui = GameUi::default();
        let world = World::new(1);
        let mut player = looking(0.0, 0.0);
        player.position = DVec3::new(12.34, 70.0, -5.06);
        let shown = |facts: &HudFacts| {
            let mut out = Vec::new();
            ui.hud(facts, &world, &player, &mut out);
            out
        };
        let mut full = facts(HudMode::Full);
        full.fps = Some(60);
        let out = shown(&full);
        assert_eq!(labels(&out)[..2], ["X: 12.3    Y: 70.0    Z: -5.1", "60 FPS"]);
        assert_eq!(out.iter().take_while(|e| matches!(e, HudElement::Rect { .. })).count(), 3, "the reticle comes first");
        full.players_online = Some(3);
        full.ping_ms = Some(42);
        full.fps = None;
        full.cruise = Some(100_000.0);
        let out = shown(&full);
        assert_eq!(
            labels(&out)[..3],
            ["X: 12.3    Y: 70.0    Z: -5.1    CRUISE 100000 km/s", "-- FPS", "players online: 3   42 ms"]
        );
        let minimal = shown(&facts(HudMode::Minimal));
        assert!(matches!(minimal[0], HudElement::Rect { .. }), "the reticle stays");
        assert!(
            !labels(&minimal).iter().any(|l| l.contains("FPS") || l.starts_with("X:")),
            "no readouts below Full: {:?}",
            labels(&minimal)
        );
        assert!(shown(&facts(HudMode::Off)).is_empty(), "HUD Off shows nothing");
    }

    #[test]
    fn loading_and_interrupted_lines_follow_the_facts() {
        let ui = GameUi::default();
        let world = World::new(1);
        let player = looking(0.0, 0.0);
        let first = |facts: &HudFacts| {
            let mut out = Vec::new();
            ui.hud(facts, &world, &player, &mut out);
            labels(&out).first().map(|s| s.to_string())
        };
        let mut f = facts(HudMode::Full);
        f.spawn_ready = false;
        assert_eq!(first(&f).as_deref(), Some("Loading terrain…"));
        f.snapshot_ready = false;
        assert_eq!(first(&f).as_deref(), Some("Loading world…"), "a join waits for the world first");
        f.hud_mode = HudMode::Off;
        assert_eq!(first(&f), None);
        let mut lost = facts(HudMode::Minimal);
        lost.link_interrupted = true;
        assert_eq!(first(&lost).as_deref(), Some("connection interrupted"));
    }

    /// The coordinates shrink to fit between the frame rate and the minimap, never below half.
    #[test]
    fn the_coordinates_fit_between_the_readouts() {
        let mut f = facts(HudMode::Full);
        assert_eq!(info::fitted_base(&f, 26, 30, 1000), 26, "room to spare");
        assert_eq!(info::fitted_base(&f, 26, 30, 600), 20, "30 glyphs of 20 px fill 600 px");
        assert_eq!(info::fitted_base(&f, 26, 30, 100), 13, "never below half the full size");
        f.ui_scale = 1.5;
        let base = info::fitted_base(&f, 26, 30, 600);
        assert!(f.font_px(base) * 30 <= 600 && f.font_px(base + 1) * 30 > 600, "the largest size that fits at scale 1.5");
    }

    /// An unchanged readout reuses its shared strings.
    #[test]
    fn unchanged_readouts_reuse_their_strings() {
        let ui = GameUi::default();
        let world = World::new(1);
        let player = looking(0.0, 0.0);
        let mut f = facts(HudMode::Full);
        f.fps = Some(60);
        f.players_online = Some(2);
        let texts = || {
            let mut out = Vec::new();
            ui.hud(&f, &world, &player, &mut out);
            out.into_iter()
                .filter_map(|e| match e {
                    HudElement::Label { text, .. } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let (a, b) = (texts(), texts());
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(x, y)| std::sync::Arc::ptr_eq(x, y)));
    }

    #[test]
    fn register_installs_game_ui() {
        let package = ModDescriptor { id: "pwc.game-ui", name: "Game UI", version: "1.0.0", register };
        let mods = GameBuild::new().with_mod(package).mods();
        assert_eq!((mods.len(), mods.id(0), mods.package(0)), (1, "game_ui", Some("pwc.game-ui")));
        assert!(mods.is_active(0));
    }
}
