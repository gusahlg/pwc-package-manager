//! The menu framework: a [`Menu`] builds a [`View`] of rows from the screen context and answers
//! one [`Msg`] at a time; [`Framed`] adds the cursor and turns it into a core [`Screen`]. Input
//! flows `MenuInput` → [`Intent`]s ([`gather`]) → a `Msg` ([`drive`]) → a [`ScreenOutcome`].

use std::cell::Cell;

use pwc_mod_api::input::intent::{EditKey, MenuEvent};
use pwc_mod_api::screen::{MenuInput, Screen, ScreenContext, ScreenOutcome, UiElement};

use crate::text::EditBuf;
use crate::theme::{present, DefaultTheme, MenuTheme};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Prev,
    Next,
}

impl Dir {
    pub fn delta(self) -> i32 {
        match self {
            Dir::Prev => -1,
            Dir::Next => 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextOp {
    Char(char),
    Backspace,
    DelWord,
    Left,
    Right,
    Home,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intent {
    Nav(Dir),
    Adjust(Dir),
    Confirm,
    Cancel,
    Edit(TextOp),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    /// Soft status (failed connect) — salmon.
    Info,
    /// Refused action (bad port) — red.
    Error,
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub text: String,
    pub level: Level,
}

impl Notice {
    pub fn info(text: String) -> Self {
        Self { text, level: Level::Info }
    }

    pub fn error(text: String) -> Self {
        Self { text, level: Level::Error }
    }
}

#[derive(Clone, Debug)]
pub enum Style {
    Title { subtitle: String },
    Panel,
}

#[derive(Clone, Debug)]
pub enum ValueView {
    Toggle(bool),
    Choice(String),
    Bar { t: f32, label: String },
}

/// Decides which Msg a Confirm/Adjust emits.
#[derive(Clone, Debug)]
pub enum RowKind {
    Action,
    Value(ValueView),
    Text { content: String, caret: usize, masked: bool },
    Heading,
}

/// Carries action `A` directly (no id ladder).
#[derive(Clone, Debug)]
pub struct Row<A: Copy> {
    pub label: String,
    pub detail: Option<String>,
    pub kind: RowKind,
    /// None means not selectable (heading/separator/disabled).
    pub tag: Option<A>,
}

impl<A: Copy> Row<A> {
    pub fn action(label: impl Into<String>, tag: A) -> Self {
        Self { label: label.into(), detail: None, kind: RowKind::Action, tag: Some(tag) }
    }

    pub fn value(label: impl Into<String>, view: ValueView, tag: A) -> Self {
        Self { label: label.into(), detail: None, kind: RowKind::Value(view), tag: Some(tag) }
    }

    pub fn text(label: impl Into<String>, content: String, caret: usize, masked: bool, tag: A) -> Self {
        Self { label: label.into(), detail: None, kind: RowKind::Text { content, caret, masked }, tag: Some(tag) }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Non-selectable section title.
    pub fn heading(label: impl Into<String>) -> Self {
        Self { label: label.into(), detail: None, kind: RowKind::Heading, tag: None }
    }
}

#[derive(Clone, Debug)]
pub struct View<A: Copy> {
    pub title: String,
    pub style: Style,
    pub rows: Vec<Row<A>>,
    /// Confirm while editing a Text row picks this (form submit).
    pub default: Option<A>,
    pub hint: String,
    pub notice: Option<Notice>,
}

impl<A: Copy> View<A> {
    pub fn is_selectable(&self, i: usize) -> bool {
        self.rows.get(i).is_some_and(|r| r.tag.is_some())
    }

    fn tag_at(&self, i: usize) -> Option<A> {
        self.rows.get(i).and_then(|r| r.tag)
    }

    fn kind_at(&self, i: usize) -> Option<&RowKind> {
        self.rows.get(i).map(|r| &r.kind)
    }
}

/// Menu's own action vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Msg<A: Copy> {
    Pick(A),
    Step(A, Dir),
    Edited(A, TextOp),
    /// A character typed while an action row is highlighted (a row shortcut, e.g. D to delete).
    Key(A, char),
    /// Navigation moved the highlight onto this row and nothing else happened this frame.
    Hover(A),
    Back,
}

/// A page of rows: a pure view and an effectful update. The answer is a core
/// [`ScreenOutcome`]: stay, go back, push a screen, open a registered entry, or ask the core
/// for something only it can do.
pub trait Menu {
    type Action: Copy;
    fn view(&self, ctx: &ScreenContext) -> View<Self::Action>;
    fn update(&mut self, msg: Msg<Self::Action>, ctx: &mut ScreenContext) -> ScreenOutcome;
}

/// Always resolvable to a selectable row.
#[derive(Default, Clone, Copy, Debug)]
pub struct Cursor {
    pub index: usize,
}

impl Cursor {
    pub fn normalize<A: Copy>(&mut self, view: &View<A>) {
        self.index = self.resolved(view);
    }

    /// Immutable resolution (for use in draw).
    pub fn resolved<A: Copy>(&self, view: &View<A>) -> usize {
        let n = view.rows.len();
        if n == 0 {
            return 0;
        }
        let start = self.index.min(n - 1);
        if view.is_selectable(start) {
            return start;
        }
        // Search outward for the nearest selectable row.
        for d in 1..n {
            if start >= d && view.is_selectable(start - d) {
                return start - d;
            }
            if start + d < n && view.is_selectable(start + d) {
                return start + d;
            }
        }
        start
    }

    fn nav<A: Copy>(&mut self, view: &View<A>, dir: Dir) {
        let n = view.rows.len();
        if n == 0 {
            return;
        }
        let step = |i: usize| match dir {
            Dir::Next => (i + 1) % n,
            Dir::Prev => (i + n - 1) % n,
        };
        let mut i = step(self.index.min(n - 1));
        for _ in 0..n {
            if view.is_selectable(i) {
                self.index = i;
                return;
            }
            i = step(i);
        }
    }
}

/// A concrete [`Menu`] plus its cursor: a core [`Screen`].
pub struct Framed<M: Menu> {
    menu: M,
    cursor: Cursor,
    /// The view an update built when no message reached the menu, kept for the frame's draw.
    view: Cell<Option<View<M::Action>>>,
}

impl<M: Menu> Framed<M> {
    pub fn new(menu: M) -> Self {
        Self { menu, cursor: Cursor::default(), view: Cell::new(None) }
    }

    /// Box a menu into a screen for [`ScreenOutcome::Push`] or an entry's `open`.
    pub fn boxed(menu: M) -> Box<dyn Screen>
    where
        M: 'static,
    {
        Box::new(Self::new(menu))
    }

    /// The menu inside.
    pub fn menu(&self) -> &M {
        &self.menu
    }

    /// The view to show and its selected row: the one the last update built if no message has
    /// reached the menu since, else a fresh one.
    pub fn view_sel(&self, ctx: &ScreenContext) -> (View<M::Action>, usize) {
        let view = self.view.take().unwrap_or_else(|| self.menu.view(ctx));
        let sel = self.cursor.resolved(&view);
        (view, sel)
    }

    /// One frame of already gathered intents (what [`Screen::update`] does with the core's
    /// input). Screens that compose pages call it directly.
    pub fn update_intents(&mut self, intents: &[Intent], ctx: &mut ScreenContext) -> ScreenOutcome {
        let view = self.menu.view(ctx);
        self.cursor.normalize(&view);
        match drive(intents, &view, &mut self.cursor) {
            Some(msg) => {
                *self.view.get_mut() = None;
                self.menu.update(msg, ctx)
            }
            None => {
                *self.view.get_mut() = Some(view);
                ScreenOutcome::Stay
            }
        }
    }

    /// Draw with `theme` (what [`Screen::draw`] does with the default theme).
    pub fn draw_with(&self, theme: &dyn MenuTheme, ctx: &ScreenContext, out: &mut Vec<UiElement>, size: (i32, i32)) {
        let (view, sel) = self.view_sel(ctx);
        theme.draw(out, present(view, ctx.settings().menu_scale), sel, size.0, size.1);
    }
}

impl<M: Menu> Screen for Framed<M> {
    fn update(&mut self, input: &MenuInput, ctx: &mut ScreenContext) -> ScreenOutcome {
        let intents = gather(input);
        self.update_intents(&intents, ctx)
    }

    fn draw(&self, ctx: &ScreenContext, out: &mut Vec<UiElement>, size: (i32, i32)) {
        self.draw_with(&DefaultTheme, ctx, out, size);
    }
}

/// This frame's menu intents from the core's input. Order matters within kinds: navigation and
/// confirm first, then edits. A quiet frame allocates nothing.
pub fn gather(input: &MenuInput) -> Vec<Intent> {
    let mut out = Vec::new();
    if input.event(MenuEvent::Up) {
        out.push(Intent::Nav(Dir::Prev));
    }
    // Tab walks forward through rows/fields, like Down.
    if input.event(MenuEvent::Down) || input.event(MenuEvent::NextTab) {
        out.push(Intent::Nav(Dir::Next));
    }
    if input.event(MenuEvent::Left) {
        out.push(Intent::Adjust(Dir::Prev));
    }
    if input.event(MenuEvent::Right) {
        out.push(Intent::Adjust(Dir::Next));
    }
    if input.event(MenuEvent::Confirm) || input.event(MenuEvent::Toggle) {
        out.push(Intent::Confirm);
    }
    if input.event(MenuEvent::Back) {
        out.push(Intent::Cancel);
    }
    for &c in input.chars() {
        out.push(Intent::Edit(TextOp::Char(c)));
    }
    if input.event(MenuEvent::Delete) {
        out.push(Intent::Edit(TextOp::Backspace));
    }
    match input.edit() {
        Some(EditKey::DelWord) => out.push(Intent::Edit(TextOp::DelWord)),
        Some(EditKey::Home) => out.push(Intent::Edit(TextOp::Home)),
        Some(EditKey::End) => out.push(Intent::Edit(TextOp::End)),
        _ => {}
    }
    out
}

/// On Text rows, editing has priority so characters don't trigger nav.
pub fn drive<A: Copy>(intents: &[Intent], view: &View<A>, cursor: &mut Cursor) -> Option<Msg<A>> {
    let n = view.rows.len();
    if n == 0 {
        return cancel(intents).then_some(Msg::Back);
    }

    let sel = cursor.index.min(n - 1);
    let on_text = matches!(view.kind_at(sel), Some(RowKind::Text { .. }));

    if on_text {
        let tag = view.tag_at(sel);
        // Edit has priority so chars/backspace don't trigger nav.
        for i in intents {
            if let Intent::Edit(op) = i
                && let Some(t) = tag
            {
                return Some(Msg::Edited(t, *op));
            }
        }
        // Adjust moves the caret.
        for i in intents {
            if let Intent::Adjust(d) = i
                && let Some(t) = tag
            {
                let op = if *d == Dir::Prev { TextOp::Left } else { TextOp::Right };
                return Some(Msg::Edited(t, op));
            }
        }
        if confirm(intents)
            && let Some(a) = view.default
        {
            return Some(Msg::Pick(a));
        }
        if cancel(intents) {
            return Some(Msg::Back);
        }
        // Nav between fields: cursor moves but yields no message.
        for i in intents {
            if let Intent::Nav(d) = i {
                cursor.nav(view, *d);
            }
        }
        return None;
    }

    // Non-text row: navigate first, then act on the row the highlight lands on.
    let before = cursor.index;
    for i in intents {
        if let Intent::Nav(d) = i {
            cursor.nav(view, *d);
        }
    }
    if cancel(intents) {
        return Some(Msg::Back);
    }
    let sel = cursor.index.min(n - 1);
    let tag = view.tag_at(sel)?;
    match view.kind_at(sel) {
        Some(RowKind::Value(_)) => {
            for i in intents {
                if let Intent::Adjust(d) = i {
                    return Some(Msg::Step(tag, *d));
                }
            }
            if confirm(intents) {
                return Some(Msg::Step(tag, Dir::Next));
            }
        }
        Some(RowKind::Action) if confirm(intents) => {
            return Some(Msg::Pick(tag));
        }
        Some(RowKind::Action) => {
            for i in intents {
                if let Intent::Edit(TextOp::Char(c)) = i {
                    return Some(Msg::Key(tag, *c));
                }
            }
        }
        _ => {}
    }
    (cursor.index != before).then_some(Msg::Hover(tag))
}

fn confirm(intents: &[Intent]) -> bool {
    intents.iter().any(|i| matches!(i, Intent::Confirm))
}

fn cancel(intents: &[Intent]) -> bool {
    intents.iter().any(|i| matches!(i, Intent::Cancel))
}

/// Shown when a port field does not parse.
pub const PORT_ERROR: &str = "invalid port (1-65535)";

/// Parse a port field. Empty means `default`; otherwise 1-65535.
pub fn parse_port(text: &str, default: u16) -> Option<u16> {
    let text = text.trim();
    if text.is_empty() {
        return Some(default);
    }
    match text.parse::<u16>() {
        Ok(0) | Err(_) => None,
        Ok(port) => Some(port),
    }
}

/// Apply one edit to a form field.
pub fn apply_text_op(buf: &mut EditBuf, op: TextOp) {
    match op {
        TextOp::Char(c) => {
            buf.insert_char(c);
        }
        TextOp::Backspace => {
            buf.backspace();
        }
        TextOp::DelWord => buf.delete_word(),
        TextOp::Left => buf.left(),
        TextOp::Right => buf.right(),
        TextOp::Home => buf.home(),
        TextOp::End => buf.end(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;

    #[test]
    fn confirm_on_a_choice_row_steps_forward_like_right() {
        let view = View {
            title: String::new(),
            style: Style::Panel,
            rows: vec![Row::value("Tile", ValueView::Choice("32".into()), 0)],
            default: None,
            hint: String::new(),
            notice: None,
        };
        let mut cursor = Cursor::default();
        assert_eq!(drive(&[Intent::Confirm], &view, &mut cursor), Some(Msg::Step(0, Dir::Next)));
        assert_eq!(drive(&[Intent::Adjust(Dir::Next)], &view, &mut cursor), Some(Msg::Step(0, Dir::Next)));
    }

    fn actions(n: usize) -> View<usize> {
        View {
            title: String::new(),
            style: Style::Panel,
            rows: (0..n).map(|i| Row::action(format!("row {i}"), i)).collect(),
            default: None,
            hint: String::new(),
            notice: None,
        }
    }

    /// A character on an action row is that row's shortcut; Enter still picks it.
    #[test]
    fn a_typed_character_on_an_action_row_is_a_key_message() {
        let view = actions(2);
        let mut cursor = Cursor::default();
        assert_eq!(drive(&[Intent::Edit(TextOp::Char('d'))], &view, &mut cursor), Some(Msg::Key(0, 'd')));
        assert_eq!(drive(&[Intent::Confirm], &view, &mut cursor), Some(Msg::Pick(0)));
    }

    /// A menu that counts the views it builds.
    struct Counted(Cell<usize>);

    impl Menu for Counted {
        type Action = usize;
        fn view(&self, _ctx: &ScreenContext) -> View<usize> {
            self.0.set(self.0.get() + 1);
            actions(2)
        }
        fn update(&mut self, _msg: Msg<usize>, _ctx: &mut ScreenContext) -> ScreenOutcome {
            ScreenOutcome::Stay
        }
    }

    /// A frame whose update reaches the menu with no message draws the view that update built.
    #[test]
    fn a_quiet_frame_builds_the_view_once() {
        let mut fixture = Fixture::new();
        let mut ctx = fixture.ctx();
        let mut framed = Framed::new(Counted(Cell::new(0)));
        let built = |f: &Framed<Counted>| f.menu.0.get();
        assert!(matches!(framed.update_intents(&[], &mut ctx), ScreenOutcome::Stay));
        assert_eq!(framed.view_sel(&ctx).1, 0);
        assert_eq!(built(&framed), 1, "the draw reuses the update's view");
        let _ = framed.update_intents(&[Intent::Nav(Dir::Next)], &mut ctx);
        assert_eq!(framed.view_sel(&ctx).1, 1);
        assert_eq!(built(&framed), 3, "a message reached the menu: the draw builds afresh");
        let _ = framed.view_sel(&ctx);
        assert_eq!(built(&framed), 4, "a draw with no update since builds its own");
    }

    /// Moving the highlight says where it landed; a frame with no movement says nothing.
    #[test]
    fn navigation_reports_the_row_it_lands_on() {
        let view = actions(3);
        let mut cursor = Cursor::default();
        assert_eq!(drive(&[Intent::Nav(Dir::Next)], &view, &mut cursor), Some(Msg::Hover(1)));
        assert_eq!(drive(&[], &view, &mut cursor), None);
        assert_eq!(drive(&[Intent::Nav(Dir::Prev)], &view, &mut cursor), Some(Msg::Hover(0)));
    }

    /// The core's input reaches the menu as intents, in order; a quiet frame allocates nothing.
    #[test]
    fn gather_turns_the_core_input_into_intents() {
        let quiet = MenuInput::new();
        assert!(gather(&quiet).is_empty());
        let input = MenuInput::new().with(MenuEvent::Down).with(MenuEvent::Toggle).with_chars(&['d']).with_edit(EditKey::Home);
        assert_eq!(
            gather(&input),
            [Intent::Nav(Dir::Next), Intent::Confirm, Intent::Edit(TextOp::Char('d')), Intent::Edit(TextOp::Home)]
        );
        assert_eq!(gather(&MenuInput::new().with(MenuEvent::Back).with(MenuEvent::Delete)), [Intent::Cancel, Intent::Edit(TextOp::Backspace)]);
    }

    #[test]
    fn ports_parse_with_an_empty_default() {
        assert_eq!(parse_port("", 5555), Some(5555));
        assert_eq!(parse_port(" 7777 ", 5555), Some(7777));
        assert_eq!(parse_port("0", 5555), None);
        assert_eq!(parse_port("70000", 5555), None);
        let mut buf = EditBuf::with("ab", 8);
        apply_text_op(&mut buf, TextOp::Left);
        apply_text_op(&mut buf, TextOp::Char('x'));
        assert_eq!((buf.text(), buf.caret_chars()), ("axb", 2));
    }
}
