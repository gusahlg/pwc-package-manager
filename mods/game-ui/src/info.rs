//! The information HUD: the reticle, the loading line, the coordinates, the frame rate, the player
//! count and the "connection interrupted" banner, from the facts the core passes to `hud`. The core
//! used to draw these itself; the layout and the text are unchanged.
//!
//! - The reticle shows in the Full and Minimal HUD modes.
//! - "Loading world…" (a server join until its snapshot lands) or "Loading terrain…" (until the
//!   spawn has loaded) replaces the readouts at the top, in Full and Minimal.
//! - Full mode only: the coordinates at the top centre (with the cruise speed while cruising),
//!   shrunk to fit between the frame rate and the minimap; the frame rate at the top left; on a
//!   server, the player count and ping under the minimap.
//! - "connection interrupted" under the top line while the server is silent, in Full and Minimal.
//!
//! Strings are formatted only when the shown value changes and are shared afterwards, so a frame
//! with nothing new allocates nothing.

use std::cell::RefCell;
use std::sync::Arc;

use pwc_mod_api::derived::Memo;
use pwc_mod_api::engine::Color;
use pwc_mod_api::player::Player;
use pwc_mod_api::ui::{Anchor, HudElement, Role};
use pwc_mod_api::HudFacts;

/// Length of each reticle arm in pixels.
const ARM: i32 = 8;
const RETICLE: Color = Color::new(255, 255, 255, 180);

/// The information HUD's string caches.
pub(crate) struct Info {
    /// The coordinate line and its length in glyphs.
    coords: RefCell<Memo<[i64; 4], (Arc<str>, i32)>>,
    /// The frame-rate readout and its length in glyphs.
    fps: RefCell<Memo<Option<u32>, (Arc<str>, i32)>>,
    online: RefCell<Memo<(usize, Option<u32>), Arc<str>>>,
    loading_world: Arc<str>,
    loading_terrain: Arc<str>,
    interrupted: Arc<str>,
}

impl Default for Info {
    fn default() -> Self {
        Self {
            coords: RefCell::new(Memo::new()),
            fps: RefCell::new(Memo::new()),
            online: RefCell::new(Memo::new()),
            loading_world: "Loading world…".into(),
            loading_terrain: "Loading terrain…".into(),
            interrupted: "connection interrupted".into(),
        }
    }
}

impl Info {
    pub(crate) fn hud(&self, facts: &HudFacts, player: &Player, out: &mut Vec<HudElement>) {
        let mode = facts.hud_mode;
        if mode.shows_world_ui() {
            reticle(facts.screen, out);
        }
        let label = |at, off, base_fs, role, text: &Arc<str>| HudElement::Label { at, off, base_fs, role, text: text.clone() };
        if !facts.snapshot_ready && mode.shows_world_ui() {
            out.push(label(Anchor::Top, (0, 12), 26, Role::Primary, &self.loading_world));
        } else if !facts.spawn_ready && mode.shows_world_ui() {
            out.push(label(Anchor::Top, (0, 12), 26, Role::Primary, &self.loading_terrain));
        } else if mode.shows_info() {
            let (fps, fps_len) = self.fps.borrow_mut().get_or(facts.fps, || counted(fps_text(facts.fps))).clone();
            let (coords, coords_len) = self.coords(facts, player);
            // The centred coordinates shrink to fit between the frame-rate readout and the minimap.
            let fps_w = 10 + fps_len * facts.font_px(20);
            let side = fps_w.max(facts.minimap_corner.0) + 12;
            let base = fitted_base(facts, 26, coords_len, facts.screen.0 - 2 * side);
            out.push(label(Anchor::Top, (0, 12), base, Role::Primary, &coords));
            out.push(label(Anchor::TopLeft, (10, 12), 20, Role::Positive, &fps));
            if let Some(count) = facts.players_online {
                let key = (count, facts.ping_ms);
                let online = self.online.borrow_mut().get_or(key, || online_text(key)).clone();
                out.push(label(Anchor::TopRight, (-12, 180), 20, Role::Positive, &online));
            }
        }
        if facts.link_interrupted && mode.shows_world_ui() {
            out.push(label(Anchor::Top, (0, 44), 22, Role::Warning, &self.interrupted));
        }
    }

    /// The coordinate line, re-formatted only when a shown digit moves (0.1 block, 1 km/s).
    fn coords(&self, facts: &HudFacts, player: &Player) -> (Arc<str>, i32) {
        let p = player.position;
        let key = [(p.x * 10.0) as i64, (p.y * 10.0) as i64, (p.z * 10.0) as i64, facts.cruise.map_or(-1, |km_s| km_s as i64)];
        let cruise = facts.cruise;
        self.coords
            .borrow_mut()
            .get_or(key, || {
                counted(match cruise {
                    Some(km_s) => format!("X: {:.1}    Y: {:.1}    Z: {:.1}    CRUISE {km_s:.0} km/s", p.x, p.y, p.z).into(),
                    None => format!("X: {:.1}    Y: {:.1}    Z: {:.1}", p.x, p.y, p.z).into(),
                })
            })
            .clone()
    }
}

/// The reticle: a horizontal bar and two vertical arms around the screen centre, one pixel thick.
fn reticle((w, h): (i32, i32), out: &mut Vec<HudElement>) {
    let (cx, cy) = (w / 2, h / 2);
    let rect = |off, size| HudElement::Rect { at: Anchor::TopLeft, off, size, color: RETICLE };
    out.push(rect((cx - ARM, cy), (2 * ARM + 1, 1)));
    out.push(rect((cx, cy - ARM), (1, ARM)));
    out.push(rect((cx, cy + 1), (1, ARM)));
}

fn fps_text(fps: Option<u32>) -> Arc<str> {
    match fps {
        Some(fps) => format!("{fps:2} FPS").into(),
        // The scripted harness pins the readout so golden shots stay reproducible.
        None => "-- FPS".into(),
    }
}

fn online_text((count, ping): (usize, Option<u32>)) -> Arc<str> {
    match ping {
        Some(ms) => format!("players online: {count}   {ms} ms").into(),
        None => format!("players online: {count}").into(),
    }
}

/// A text with its length in glyphs, counted once when it is formatted.
fn counted(text: Arc<str>) -> (Arc<str>, i32) {
    let len = text.chars().count() as i32;
    (text, len)
}

/// The largest label size (as a `base_fs`, at most `base_fs`) at which `chars` glyphs fit in
/// `max_w` pixels, never below half the full size. Every glyph advances by its font size, so
/// the width is exact.
pub(crate) fn fitted_base(facts: &HudFacts, base_fs: i32, chars: i32, max_w: i32) -> i32 {
    let full = facts.font_px(base_fs);
    let fits = |base: i32| facts.font_px(base) * chars.max(1) <= max_w;
    if max_w <= 0 || fits(base_fs) {
        return base_fs;
    }
    let mut base = base_fs;
    while base > 1 && !fits(base) && facts.font_px(base - 1) >= full / 2 {
        base -= 1;
    }
    base
}
