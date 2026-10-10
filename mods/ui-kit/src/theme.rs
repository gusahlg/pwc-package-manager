//! The default look: lays out a presented view and turns it into the core's closed set of
//! [`UiElement`]s. Mods never draw; the core renders what this produces.
//!
//! The font is monospace with an advance of exactly the font size per glyph, so
//! [`text_width`] is the whole layout metric.

use std::borrow::Cow;

use pwc_mod_api::engine::Color;
use pwc_mod_api::screen::{text_width, Phase, UiElement};
use pwc_mod_api::ui::{Anchor, Role};

use crate::menu::{Level, Notice, RowKind, Style, ValueView, View};

/// Background of every screen outside a world.
pub const MENU_BG: Color = Color::new(18, 20, 28, 255);

/// Untyped row for rendering; type tag dropped.
#[derive(Clone, Debug)]
pub struct PresentedRow {
    pub label: String,
    pub detail: Option<String>,
    pub kind: RowKind,
    pub selectable: bool,
}

/// Screen ready to render.
#[derive(Clone, Debug)]
pub struct PresentedView {
    pub title: String,
    pub style: Style,
    pub rows: Vec<PresentedRow>,
    /// User base scale (`Settings::menu_scale`); themes still shrink to fit.
    pub scale: f32,
    pub hint: String,
    pub notice: Option<Notice>,
}

/// The untyped screen at `scale`. A view passed by value moves its strings; a borrowed one is
/// cloned.
pub fn present(view: impl Into<PresentedView>, scale: f32) -> PresentedView {
    PresentedView { scale, ..view.into() }
}

/// At scale 1.
impl<A: Copy> From<View<A>> for PresentedView {
    fn from(view: View<A>) -> Self {
        let rows = view
            .rows
            .into_iter()
            .map(|r| PresentedRow { label: r.label, detail: r.detail, kind: r.kind, selectable: r.tag.is_some() })
            .collect();
        Self { title: view.title, style: view.style, rows, scale: 1.0, hint: view.hint, notice: view.notice }
    }
}

impl<A: Copy> From<&View<A>> for PresentedView {
    fn from(view: &View<A>) -> Self {
        view.clone().into()
    }
}

/// A row's screen rectangle — the geometry drawing and hit-testing share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A theme: layout and drawing, with the default for both.
pub trait MenuTheme {
    fn layout(&self, v: &PresentedView, w: i32, h: i32) -> Vec<RowRect> {
        default_layout(v, w, h)
    }

    fn draw(&self, out: &mut Vec<UiElement>, v: PresentedView, sel: usize, w: i32, h: i32) {
        default_draw(out, v, sel, w, h);
    }
}

/// The standard look of every screen outside a world.
pub struct DefaultTheme;

impl MenuTheme for DefaultTheme {}

// Shared metrics.

/// Bottom strip reserved for the hint and notice lines.
const RESERVE: i32 = 60;

struct Metrics {
    fs: i32,
    line_h: i32,
    start_y: i32,
    /// Left edge for panel rows; centred styles ignore it.
    x: i32,
    centered: bool,
}

/// Base font size scaled by the user's menu scale, then capped so every row
/// fits on screen (the cap wins over the scale; floor keeps text legible).
fn fit_fs(base: i32, scale: f32, cap: i32) -> i32 {
    ((base as f32 * scale) as i32).min(cap).max(8)
}

fn notice_line_count(v: &PresentedView) -> usize {
    v.notice.as_ref().map(|n| n.text.lines().filter(|l| !l.is_empty()).count()).unwrap_or(0)
}

fn bottom_reserve(v: &PresentedView) -> i32 {
    RESERVE + notice_line_count(v).saturating_sub(1) as i32 * 26
}

fn metrics(v: &PresentedView, w: i32, h: i32) -> Metrics {
    let n = v.rows.len().max(1) as i32;
    let reserve = bottom_reserve(v);
    match &v.style {
        Style::Title { .. } => {
            // Rows hang from mid-screen; the cap keeps the last row above the
            // hint strip: (n-2)·line_h + fs ≤ h/2 - reserve, line_h = 3·fs/2.
            let cap = (h - 2 * reserve) / (3 * n.max(2) - 4);
            let fs = fit_fs(28, v.scale, cap);
            let line_h = fs * 3 / 2;
            Metrics { fs, line_h, start_y: h / 2 - line_h, x: 0, centered: true }
        }
        Style::Panel => {
            // Rows are centred between the title zone (title_fs = fs + 14 at
            // h/8) and the hint strip; the cap solves n·(7·fs/4) ≤ that region.
            let cap = (7 * h - 8 * (reserve + 26)) / (14 * n + 8);
            let fs = fit_fs(26, v.scale, cap);
            let line_h = fs * 7 / 4;
            let top = h / 8 + (fs + 14) + 12;
            let start_y = top + (h - reserve - top - n * line_h).max(0) / 2;
            Metrics { fs, line_h, start_y, x: w / 2 - fs * 10, centered: false }
        }
    }
}

fn default_layout(v: &PresentedView, w: i32, h: i32) -> Vec<RowRect> {
    let m = metrics(v, w, h);
    v.rows
        .iter()
        .enumerate()
        .map(|(i, _)| RowRect {
            x: if m.centered { 0 } else { m.x },
            y: m.start_y + m.line_h * i as i32,
            w: if m.centered { w } else { m.fs * 20 },
            h: m.fs,
        })
        .collect()
}

/// A line of text with the 1px drop shadow every UI string has.
pub fn shadowed(out: &mut Vec<UiElement>, text: impl Into<Cow<'static, str>>, x: i32, y: i32, size: i32, color: Color) {
    out.push(UiElement::Text { x, y, size, color, text: text.into(), shadow: true });
}

/// A shadowed line placed by an anchor on the screen, the way the core's HUD labels are.
#[allow(clippy::too_many_arguments)] // placement, size, colour and text are all independent
pub fn label(
    out: &mut Vec<UiElement>,
    screen: (i32, i32),
    at: Anchor,
    off: (i32, i32),
    size: i32,
    color: Color,
    text: impl Into<Cow<'static, str>>,
) {
    let text = text.into();
    let (x, y) = at.origin(screen, (text_width(&text, size), size), off);
    shadowed(out, text, x, y, size, color);
}

// Drawing.

fn default_draw(out: &mut Vec<UiElement>, mut v: PresentedView, sel: usize, w: i32, h: i32) {
    out.push(UiElement::Rect { x: 0, y: 0, w, h, color: MENU_BG });
    let m = metrics(&v, w, h);
    let notice = v.notice.take();
    let hint = std::mem::take(&mut v.hint);
    match std::mem::replace(&mut v.style, Style::Panel) {
        Style::Title { subtitle } => draw_title(out, v, subtitle, &m, sel, w, h),
        Style::Panel => draw_panel(out, v, &m, sel, w, h),
    }
    draw_notice(out, notice, w, h);
    draw_hint(out, hint, w, h);
}

fn draw_title(out: &mut Vec<UiElement>, v: PresentedView, subtitle: String, m: &Metrics, sel: usize, w: i32, h: i32) {
    let title_fs = 48;
    let tx = (w - text_width(&v.title, title_fs)) / 2;
    shadowed(out, v.title, tx, h / 6, title_fs, Color::GOLD);

    let sub_fs = 20;
    let sx = (w - text_width(&subtitle, sub_fs)) / 2;
    shadowed(out, subtitle, sx, h / 6 + title_fs + 8, sub_fs, Color::GRAY);

    for (i, row) in v.rows.into_iter().enumerate() {
        let selected = i == sel;
        let color = row_color(&row, selected);
        let text = row_body(&row, selected);
        let x = (w - text_width(&text, m.fs)) / 2;
        shadowed(out, text, x, m.start_y + m.line_h * i as i32, m.fs, color);
    }
}

fn draw_panel(out: &mut Vec<UiElement>, v: PresentedView, m: &Metrics, sel: usize, w: i32, h: i32) {
    let title_fs = m.fs + 14;
    let tx = (w - text_width(&v.title, title_fs)) / 2;
    shadowed(out, v.title, tx, h / 8, title_fs, Color::GOLD);

    if v.rows.is_empty() {
        shadowed(out, "  (nothing here)", m.x, m.start_y, m.fs, Color::GRAY);
    }
    let detail_fs = (m.line_h - m.fs - 2).clamp(8, 16);
    let panel_w = m.fs * 20;
    let indent = m.fs * 3 / 2;
    for (i, row) in v.rows.into_iter().enumerate() {
        let selected = i == sel;
        let y = m.start_y + m.line_h * i as i32;
        let color = row_color(&row, selected);
        shadowed(out, row_body(&row, selected), m.x, y, m.fs, color);
        if let Some(detail) = row.detail {
            let clipped = clip_detail(detail, panel_w, indent, detail_fs);
            shadowed(out, clipped, m.x + indent, y + m.fs + 2, detail_fs, Color::DARKGRAY);
        }
    }
}

/// Glyphs that fit in the panel after the detail indent (font advances `fs` px). The string
/// moves through when it fits.
fn clip_detail(detail: String, panel_w: i32, indent: i32, detail_fs: i32) -> String {
    let max = (panel_w.saturating_sub(indent).max(0) / detail_fs.max(1)) as usize;
    match pwc_mod_api::ui::ellipsize(&detail, max) {
        Cow::Borrowed(_) => detail,
        Cow::Owned(clipped) => clipped,
    }
}

fn row_body(row: &PresentedRow, selected: bool) -> String {
    let mark = if selected { ">" } else { " " };
    match &row.kind {
        RowKind::Heading => row.label.clone(),
        RowKind::Action => format!("{} {}", mark, row.label),
        RowKind::Value(ValueView::Toggle(on)) => format!("{} {} {}", mark, if *on { "[x]" } else { "[ ]" }, row.label),
        RowKind::Value(ValueView::Choice(value)) => format!("{} {}: < {} >", mark, row.label, value),
        RowKind::Value(ValueView::Bar { t, label }) => format!("{} {}: {} {}", mark, row.label, bar(*t), label),
        RowKind::Text { content, caret, masked } => {
            let shown: Vec<char> = if *masked {
                std::iter::repeat_n('*', content.chars().count()).collect()
            } else {
                content.chars().collect()
            };
            let body: String = if selected {
                let at = (*caret).min(shown.len());
                shown[..at].iter().chain(&['_']).chain(&shown[at..]).collect()
            } else {
                shown.into_iter().collect()
            };
            format!("{} {}: {}", mark, row.label, body)
        }
    }
}

/// Fixed-width bar for consistent UI layout.
fn bar(t: f32) -> String {
    const CELLS: usize = 10;
    let filled = (t.clamp(0.0, 1.0) * CELLS as f32).round() as usize;
    let mut s = String::with_capacity(CELLS + 2);
    s.push('[');
    for i in 0..CELLS {
        s.push(if i < filled { '#' } else { '-' });
    }
    s.push(']');
    s
}

fn row_color(row: &PresentedRow, selected: bool) -> Color {
    if matches!(row.kind, RowKind::Heading) {
        Color::GOLD
    } else if !row.selectable {
        Color::DARKGRAY
    } else if selected {
        Color::RAYWHITE
    } else {
        Color::GRAY
    }
}

fn draw_hint(out: &mut Vec<UiElement>, hint: String, w: i32, h: i32) {
    if hint.is_empty() {
        return;
    }
    let hint_fs = 18;
    let hx = (w - text_width(&hint, hint_fs)) / 2;
    shadowed(out, hint, hx, h - 40, hint_fs, Color::DARKGRAY);
}

fn draw_notice(out: &mut Vec<UiElement>, notice: Option<Notice>, w: i32, h: i32) {
    let Some(notice) = notice else { return };
    let fs = 18;
    let color = match notice.level {
        Level::Info => Color::SALMON,
        Level::Error => Color::RED,
    };
    let lines = notice.text.lines().filter(|l| !l.is_empty());
    let count = lines.clone().count();
    let step = fs + 8;
    for (i, line) in lines.enumerate() {
        let x = (w - text_width(line, fs)) / 2;
        let from_bottom = (count - i) as i32 * step;
        shadowed(out, line.to_string(), x, h - 40 - from_bottom, fs, color);
    }
}

/// The page a screen shows while the core connects or loads a world: what is happening, and
/// that Esc cancels. Nothing for [`Phase::Idle`].
pub fn draw_waiting(out: &mut Vec<UiElement>, phase: Phase, size: (i32, i32)) {
    let title = match phase {
        Phase::Idle => return,
        Phase::Connecting => "Connecting…",
        Phase::Loading => "Loading…",
    };
    out.push(UiElement::Rect { x: 0, y: 0, w: size.0, h: size.1, color: MENU_BG });
    label(out, size, Anchor::Center, (0, -16), 28, Role::Primary.color(), title);
    label(out, size, Anchor::Center, (0, 24), 20, Role::Muted.color(), "Cancel");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::Row;

    #[test]
    fn clip_detail_fits_panel_width() {
        // 100px panel, 0 indent, 10px glyphs → 10 characters.
        assert_eq!(clip_detail("short".into(), 100, 0, 10), "short");
        assert_eq!(clip_detail("abcdefghijk".into(), 100, 0, 10), "abcdefg...");
        let long = "Start screen, menus, inventory, crafting, look and worldgen.";
        let clipped = clip_detail(long.into(), 20 * 26, 26 * 3 / 2, 16);
        let max = ((20 * 26 - 26 * 3 / 2) / 16) as usize;
        assert!(clipped.chars().count() <= max);
    }

    fn sample_view() -> View<usize> {
        View {
            title: "SAMPLE".to_string(),
            style: Style::Title { subtitle: "a test screen".to_string() },
            rows: vec![Row::heading("Section"), Row::action("First", 1), Row::action("Second", 2).detail("more")],
            default: None,
            hint: "Up/Down select".to_string(),
            notice: Some(Notice::info("one\ntwo".to_string())),
        }
    }

    fn texts(out: &[UiElement]) -> Vec<&str> {
        out.iter()
            .filter_map(|e| match e {
                UiElement::Text { text, .. } => Some(&**text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_default_theme_draws_the_page_as_elements_over_the_menu_background() {
        let mut out = Vec::new();
        DefaultTheme.draw(&mut out, present(sample_view(), 1.0), 1, 1280, 720);
        assert!(matches!(out[0], UiElement::Rect { x: 0, y: 0, w: 1280, h: 720, color } if color == MENU_BG));
        assert_eq!(texts(&out), ["SAMPLE", "a test screen", "Section", "> First", "  Second", "one", "two", "Up/Down select"]);
        let first = out.iter().find(|e| matches!(e, UiElement::Text { text, .. } if text == "> First")).unwrap();
        assert!(matches!(first, UiElement::Text { color, shadow: true, .. } if *color == Color::RAYWHITE));
        let rects = DefaultTheme.layout(&present(sample_view(), 1.0), 1280, 720);
        assert_eq!(rects.len(), 3, "one rectangle per row");
        assert!(rects.windows(2).all(|w| w[0].y < w[1].y), "rows stack top to bottom: {rects:?}");
    }

    #[test]
    fn rows_show_their_values_like_the_old_menus() {
        let row = |kind| PresentedRow { label: "Bloom".into(), detail: None, kind, selectable: true };
        assert_eq!(row_body(&row(RowKind::Value(ValueView::Toggle(true))), true), "> [x] Bloom");
        assert_eq!(row_body(&row(RowKind::Value(ValueView::Choice("4x".into()))), false), "  Bloom: < 4x >");
        assert_eq!(row_body(&row(RowKind::Value(ValueView::Bar { t: 0.5, label: "50%".into() })), false), "  Bloom: [#####-----] 50%");
        let text = RowKind::Text { content: "abc".into(), caret: 1, masked: true };
        assert_eq!(row_body(&row(text), true), "> Bloom: *_**");
    }

    #[test]
    fn the_waiting_page_says_what_is_happening_and_how_to_cancel() {
        let mut out = Vec::new();
        draw_waiting(&mut out, Phase::Idle, (1280, 720));
        assert!(out.is_empty());
        draw_waiting(&mut out, Phase::Connecting, (1280, 720));
        assert_eq!(texts(&out), ["Connecting…", "Cancel"]);
        out.clear();
        draw_waiting(&mut out, Phase::Loading, (1280, 720));
        assert_eq!(texts(&out), ["Loading…", "Cancel"]);
        let (x, y) = match &out[1] {
            UiElement::Text { x, y, .. } => (*x, *y),
            _ => panic!("a label"),
        };
        assert_eq!((x, y), ((1280 - 8 * 28) / 2, (720 - 28) / 2 - 16), "centred like the core's label");
    }
}
