//! Chat: the text chat and the scrollback of the game's messages.
//!
//! - **§** (the key left of 1; `` ` `` on a US layout) opens the chat line; Enter sends, Esc
//!   closes, and § on an empty line closes it too. Up and Down recall what you sent, Tab completes
//!   when a mod offers completions.
//! - A sent line is offered to the line handlers other mods registered through the [`ChatHandle`],
//!   in registration order; the first that takes it answers with lines for the scrollback
//!   (`pwc.commands` takes `/` commands this way). A line nobody takes is chat: on a server it goes
//!   to the players near you, or to everyone with a leading `!`; in single player it is echoed.
//! - The scrollback shows chat (a gold `[global]` tag, a blue `<name>`, the message muted), joins
//!   and leaves, and the game's notices (saves, audio faults, a lost connection). The newest six
//!   lines sit above the chat line in the Full HUD mode, and whenever the chat is open.
//!
//! The chat knows nothing of commands: [`ChatHandle`] is the whole contract.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use pwc_mod_api::derived::Memo;
use pwc_mod_api::engine::{Color, Key};
use pwc_mod_api::input::intent::{Chord, EditKey};
use pwc_mod_api::player::Player;
use pwc_mod_api::ui::{Anchor, HudElement, HudMode, Line, Role};
use pwc_mod_api::world::World;
use pwc_mod_api::{
    Action, Channel, FrameContext, GameContext, HudFacts, Message, Mod, ModRegistrar, NoticeLevel, ESSENTIALS,
};

pub mod text;

pub use text::{common_prefix, Completion};
use text::{Ring, TextInput};

#[cfg(test)]
mod tests;

/// The id of the action that opens the chat.
pub const OPEN: &str = "chat.open";
const OPEN_CHORDS: &[Chord] = &[Chord::key(Key::Backquote)];
const ACTIONS: &[Action] = &[Action {
    id: OPEN,
    label: "Open chat (§, the key left of 1)",
    default: OPEN_CHORDS,
    repeat: false,
    held: false,
    immediate: true,
}];

/// Longest line the chat accepts.
pub const MAX_INPUT: usize = 128;
/// Scrollback lines shown above the chat line.
pub const SHOWN_LINES: usize = 6;
/// Scrollback lines kept.
const KEPT_LINES: usize = SHOWN_LINES * 4;
/// The characters the chat key types on common layouts: § closes an empty line.
const CLOSE_CHARS: [char; 2] = ['§', '`'];

/// What a line handler did with a sent line.
pub enum Handled {
    /// The handler took the line; these lines go to the scrollback (none is fine).
    Consumed(Vec<Line>),
    /// Not this handler's line: offer it to the next, and send it as chat if nobody takes it.
    Pass,
}

/// A line handler: gets each sent line and the game state, in registration order.
pub type Handler = Box<dyn FnMut(&str, &mut GameContext) -> Handled>;
/// A Tab completer for the chat line.
pub type Completer = Box<dyn Fn(&str) -> Completion>;

/// State the chat shares with the mods that hold a [`ChatHandle`].
struct Shared {
    log: Ring<Line>,
    /// Bumped on every scrollback change, so the HUD rebuilds only then.
    log_rev: u64,
    handlers: Vec<Handler>,
    completer: Option<Rc<dyn Fn(&str) -> Completion>>,
    /// A prefix to open the chat with on its next frame ([`ChatHandle::open_with`]).
    open_with: Option<String>,
}

impl Shared {
    fn print(&mut self, line: Line) {
        self.log.push(line);
        self.log_rev = self.log_rev.wrapping_add(1);
    }
}

/// The chat's handle for other mods, provided at registration: get it with
/// `registrar.get::<pwc_chat::ChatHandle>()` from a package that depends on `pwc.chat`.
#[derive(Clone)]
pub struct ChatHandle {
    shared: Rc<RefCell<Shared>>,
}

impl ChatHandle {
    /// Offer every sent line to `handler`, after the handlers registered before it.
    pub fn add_handler(&self, handler: impl FnMut(&str, &mut GameContext) -> Handled + 'static) {
        self.shared.borrow_mut().handlers.push(Box::new(handler));
    }

    /// Answer Tab on the chat line with `completer` (the last one set wins).
    pub fn set_completer(&self, completer: impl Fn(&str) -> Completion + 'static) {
        self.shared.borrow_mut().completer = Some(Rc::new(completer));
    }

    /// Open the chat on its next frame with `prefix` already typed (the commands key opens it
    /// with `/`). Ignored while the chat is open.
    pub fn open_with(&self, prefix: &str) {
        self.shared.borrow_mut().open_with = Some(prefix.to_string());
    }

    /// Add a line to the scrollback.
    pub fn print(&self, line: Line) {
        self.shared.borrow_mut().print(line);
    }
}

/// The package entry point: installs the chat and provides its [`ChatHandle`].
pub fn register(registrar: &mut ModRegistrar) {
    let chat = Chat::default();
    registrar.provide(chat.handle());
    registrar.add(chat);
}

/// The chat mod (id `chat`).
pub struct Chat {
    shared: Rc<RefCell<Shared>>,
    open: bool,
    input: TextInput,
    /// Bumped on every change to the chat line, so the HUD rebuilds only then.
    input_rev: u64,
    hud: RefCell<Memo<HudKey, Vec<HudElement>>>,
}

impl Default for Chat {
    fn default() -> Self {
        let shared = Shared { log: Ring::new(KEPT_LINES), log_rev: 0, handlers: Vec::new(), completer: None, open_with: None };
        Self {
            shared: Rc::new(RefCell::new(shared)),
            open: false,
            input: TextInput::new(MAX_INPUT),
            input_rev: 0,
            hud: RefCell::new(Memo::new()),
        }
    }
}

impl Chat {
    /// A handle on this chat's shared state.
    pub fn handle(&self) -> ChatHandle {
        ChatHandle { shared: self.shared.clone() }
    }

    /// Whether the chat line is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The chat line as typed so far.
    pub fn line(&self) -> &str {
        self.input.text()
    }

    /// The scrollback's text, oldest first (each line's spans joined).
    pub fn scrollback(&self) -> Vec<String> {
        let shared = self.shared.borrow();
        let mut lines: Vec<String> = shared.log.iter_rev().map(|l| l.spans().map(|s| s.text.as_str()).collect()).collect();
        lines.reverse();
        lines
    }

    fn print(&self, line: Line) {
        self.shared.borrow_mut().print(line);
    }

    fn open_line(&mut self, ctx: &mut FrameContext, prefix: &str) {
        if ctx.capture_text(true) {
            self.open = true;
            self.input.clear();
            if !prefix.is_empty() {
                self.input.set(prefix);
            }
            self.touch();
        }
    }

    fn close(&mut self, ctx: &mut FrameContext) {
        ctx.capture_text(false);
        self.open = false;
        self.input.clear();
        self.touch();
    }

    fn touch(&mut self) {
        self.input_rev = self.input_rev.wrapping_add(1);
    }

    /// Tab: ask the completer for the current line and show ambiguous candidates.
    fn complete(&mut self) {
        let completer = self.shared.borrow().completer.clone();
        if let Some(completer) = completer {
            self.input.complete(|line| completer(line));
        }
        if let Some(candidates) = self.input.take_notice() {
            self.print(Line::of(Role::Dim, candidates.join("   ")));
        }
    }

    /// A sent line: the first handler that takes it answers; otherwise it is chat.
    fn submit(&mut self, line: String, game: &mut GameContext) {
        // The handlers run outside the borrow, so one may print through its handle.
        let mut handlers = std::mem::take(&mut self.shared.borrow_mut().handlers);
        let mut answer = None;
        for handler in &mut handlers {
            if let Handled::Consumed(lines) = handler(&line, game) {
                answer = Some(lines);
                break;
            }
        }
        {
            let mut shared = self.shared.borrow_mut();
            // Keep any handler added while these ran, after the existing ones.
            handlers.append(&mut shared.handlers);
            shared.handlers = handlers;
        }
        match answer {
            Some(lines) => lines.into_iter().for_each(|l| self.print(l)),
            None => send(&line, game, |l| self.print(l)),
        }
    }
}

/// Chat out: local, or global with a leading `!`. The server echoes it back as a message, so it
/// shows when it returns. Single player has no wire: echo it.
fn send(line: &str, game: &mut GameContext, mut print: impl FnMut(Line)) {
    let (channel, text) = match line.strip_prefix('!') {
        Some(rest) => (Channel::Global, rest.trim()),
        None => (Channel::Local, line),
    };
    if text.is_empty() {
        return;
    }
    if game.networked {
        game.send_chat(channel, text);
    } else {
        print(Line::of(Role::Accent, format!("> {line}")));
    }
}

/// A chat line as the scrollback shows it: a gold `[global]` tag on global chat, a blue `<name>`
/// and the message muted.
pub fn chat_line(from: &str, channel: Channel, text: &str) -> Line {
    let name = format!("<{from}> ");
    let line = match channel {
        Channel::Global => Line::of(Role::Warning, "[global] ").then(Role::Accent, name),
        Channel::Local => Line::of(Role::Accent, name),
    };
    line.then(Role::Muted, text.to_string())
}

impl Mod for Chat {
    fn name(&self) -> &str {
        "Chat"
    }

    fn id(&self) -> &'static str {
        "chat"
    }

    fn description(&self) -> &str {
        "Text chat on § (the key left of 1), and the game's messages."
    }

    fn group(&self) -> &'static str {
        ESSENTIALS
    }

    fn actions(&self) -> &[Action] {
        ACTIONS
    }

    /// Per world: the old world's lines and an open chat line do not carry over.
    fn reset(&mut self) {
        self.open = false;
        self.input = TextInput::new(MAX_INPUT);
        self.touch();
        let mut shared = self.shared.borrow_mut();
        shared.log = Ring::new(KEPT_LINES);
        shared.log_rev = shared.log_rev.wrapping_add(1);
        shared.open_with = None;
    }

    fn on_frame(&mut self, ctx: &mut FrameContext) {
        let pending = self.shared.borrow_mut().open_with.take();
        if !self.open {
            if ctx.action(OPEN) {
                self.open_line(ctx, "");
            } else if let Some(prefix) = pending {
                self.open_line(ctx, &prefix);
            }
            return;
        }
        // Escape ended the capture (or it was lost): the chat line closes with it.
        let Some(text) = ctx.text() else {
            if !ctx.capturing() {
                self.close(ctx);
            }
            return;
        };
        if text.escape {
            self.close(ctx);
            return;
        }
        if self.input.text().is_empty() && text.edit.is_none() && matches!(text.chars, [c] if CLOSE_CHARS.contains(c)) {
            self.close(ctx);
            return;
        }
        if text.chars.is_empty() && text.edit.is_none() {
            return;
        }
        let submitted = self.input.handle(text.chars, text.edit);
        if text.edit == Some(EditKey::Complete) {
            self.complete();
        }
        self.touch();
        if let Some(line) = submitted {
            self.close(ctx);
            self.submit(line, &mut ctx.game);
        }
    }

    fn on_message(&mut self, msg: &Message) -> bool {
        let line = match *msg {
            Message::Chat { from, channel, text } => chat_line(from, channel, text),
            Message::Joined { name } => Line::of(Role::Positive, format!("* {name} joined")),
            Message::Left { name } => Line::of(Role::Muted, format!("* {name} left")),
            Message::Notice(notice) => {
                let role = match notice.level {
                    NoticeLevel::Info => Role::Dim,
                    NoticeLevel::Warning => Role::Warning,
                    NoticeLevel::Error => Role::Danger,
                };
                Line::of(role, notice.text.clone())
            }
        };
        self.print(line);
        true
    }

    fn hud(&self, facts: &HudFacts, _world: &World, _player: &Player, out: &mut Vec<HudElement>) {
        let show_log = self.open || facts.hud_mode == HudMode::Full;
        if !show_log {
            return;
        }
        let key = HudKey {
            screen: facts.screen,
            fs: facts.font_px(FONT),
            open: self.open,
            log_rev: self.shared.borrow().log_rev,
            input_rev: self.input_rev,
        };
        let mut memo = self.hud.borrow_mut();
        out.extend(memo.get_or(key, || self.paint(facts)).iter().cloned());
    }
}

/// The chat's base font size (scaled by the UI scale like every label).
const FONT: i32 = 20;
const LINE_GAP: i32 = 4;
const LEFT: i32 = 12;
const INPUT_BG: Color = Color::new(0, 0, 0, 150);
const CARET: Color = Color::YELLOW;

/// Everything the chat's HUD shows: its cache key.
#[derive(Clone, Copy, PartialEq, Debug)]
struct HudKey {
    screen: (i32, i32),
    fs: i32,
    open: bool,
    log_rev: u64,
    input_rev: u64,
}

impl Chat {
    /// The scrollback above the chat line, newest lowest, and the line itself with a caret while
    /// open. Labels are anchored bottom-left; every glyph advances by the font size, so each span
    /// starts where the one before it ends.
    fn paint(&self, facts: &HudFacts) -> Vec<HudElement> {
        let (w, h) = facts.screen;
        let fs = facts.font_px(FONT);
        let line_h = fs + LINE_GAP;
        let input_y = h - line_h - 10;
        // A label of size `fs` drawn with its top at `y`, anchored bottom-left.
        let off = |x: i32, y: i32| (x, y + fs - h);
        let mut out = Vec::new();
        let shared = self.shared.borrow();
        for (i, line) in shared.log.iter_rev().take(SHOWN_LINES).enumerate() {
            let y = input_y - line_h * (i as i32 + 1) - 6;
            let mut x = LEFT;
            for span in line.spans() {
                let text: Arc<str> = span.text.as_str().into();
                out.push(HudElement::Label { at: Anchor::BottomLeft, off: off(x, y), base_fs: FONT, role: span.role, text });
                x += span.text.chars().count() as i32 * fs;
            }
        }
        if self.open {
            out.push(HudElement::Rect {
                at: Anchor::BottomLeft,
                off: (8, input_y - 4 + line_h + 6 - h),
                size: (w - 16, line_h + 6),
                color: INPUT_BG,
            });
            let prompt = "> ";
            let shown: Arc<str> = format!("{prompt}{}", self.input.text()).into();
            out.push(HudElement::Label { at: Anchor::BottomLeft, off: off(LEFT, input_y), base_fs: FONT, role: Role::Warning, text: shown });
            let left = prompt.chars().count() + self.input.text()[..self.input.cursor()].chars().count();
            let caret_x = LEFT + left as i32 * fs;
            out.push(HudElement::Rect { at: Anchor::BottomLeft, off: (caret_x, input_y + fs - h), size: (2, fs), color: CARET });
        }
        out
    }
}
