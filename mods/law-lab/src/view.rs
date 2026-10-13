//! The lab's HUD: the running law's name, tick and books, and a heat map of one slice of its box
//! (the ground under the player by default), seen from above. Matter at rest is grey, void is
//! not drawn, and activity climbs from dark red through yellow to white. The player's column is
//! outlined.

use std::sync::Arc;

use pwc_law_core::{Grid, VOID};
use pwc_mod_api::engine::Color;
use pwc_mod_api::player::Player;
use pwc_mod_api::ui::{Anchor, HudElement, Role};
use pwc_mod_api::HudFacts;

use crate::Run;

const LEFT: i32 = 12;
const TOP: i32 = 250;
const REST: Color = Color::new(78, 80, 92, 190);
const MARK: Color = Color::new(120, 220, 255, 230);
const BACKDROP: Color = Color::new(8, 10, 14, 150);

/// Heat ramp: 1 = dark red .. 255 = white.
pub fn heat(v: u8) -> Color {
    let t = u32::from(v);
    let r = (60 + t * 3).min(255) as u8;
    let g = (t.saturating_sub(85) * 3).min(255) as u8;
    let b = (t.saturating_sub(170) * 3).min(255) as u8;
    Color::new(r, g, b, 235)
}

/// The box coordinates of slice cell `(u, v)` at `layer` along the up axis.
fn cell(run: &Run, layer: u32, u: u32, v: u32) -> (u32, u32, u32) {
    match run.window.up {
        0 => (layer, v, u),
        1 => (u, layer, v),
        _ => (u, v, layer),
    }
}

/// Paint the run.
pub fn paint(run: &Run, facts: &HudFacts, _player: &Player, out: &mut Vec<HudElement>) {
    let side = run.window.side;
    let h = side as i32 / 2;
    let s = if run.window.up_positive { 1 } else { -1 };
    let layer = (h + s * (run.depth - 1)).clamp(0, side as i32 - 1) as u32;
    let px = ((320 / side as i32).clamp(2, 10) as f32 * facts.ui_scale.max(1.0)) as i32;
    let span = px * side as i32;
    let label = |off: (i32, i32), fs: i32, role: Role, text: String| HudElement::Label {
        at: Anchor::TopLeft,
        off,
        base_fs: fs,
        role,
        text: Arc::from(text),
    };
    out.push(HudElement::Rect { at: Anchor::TopLeft, off: (LEFT - 4, TOP - 4), size: (span + 8, span + 8), color: BACKDROP });
    let law = &run.law;
    let grid: Grid = law.matter().grid;
    for v in 0..side {
        for u in 0..side {
            let (x, y, z) = cell(run, layer, u, v);
            let i = grid.idx(x, y, z);
            let heat_v = law.show(i);
            let color = match (heat_v, law.matter().get(i)) {
                (0, VOID) => continue,
                (0, _) => REST,
                (t, _) => heat(t),
            };
            out.push(HudElement::Rect {
                at: Anchor::TopLeft,
                off: (LEFT + u as i32 * px, TOP + v as i32 * px),
                size: (px, px),
                color,
            });
        }
    }
    let c = h * px;
    out.push(HudElement::Rect { at: Anchor::TopLeft, off: (LEFT + c, TOP + c), size: (px, px / 3 + 1), color: MARK });
    let mut y = TOP + span + 10;
    out.push(label((LEFT, y), 20, Role::Primary, format!("{} — tick {}", run.entry.name, run.ticks)));
    y += 24;
    let state = if run.speed == 0 { "paused".to_string() } else { format!("x{}", run.speed) };
    out.push(label((LEFT, y), 16, Role::Dim, format!("{} active cells, {state}, slice {}", law.active(), run.depth)));
    for (name, value) in law.ledger().entries.iter().take(6) {
        y += 20;
        out.push(label((LEFT, y), 16, Role::Dim, format!("{name} = {value}")));
    }
}
