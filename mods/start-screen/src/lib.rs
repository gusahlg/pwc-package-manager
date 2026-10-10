//! The default start screen: the root screen the game shows out of a world.
//!
//! Main menu (New World, Worlds, Host Server, Join Server, the entries other packages offer for the
//! main menu, Quit), the worlds page (one row per saved world), the host and join forms, and the
//! waiting page while the game connects or loads. It asks the core for what only the core can do
//! ([`AppRequest`]) and opens other packages' screens by entry id, without knowing them: a
//! settings menu or a mod list appears here because its package registered an entry.

use pwc_mod_api::net::{DEFAULT_PORT, MAX_NAME};
use pwc_mod_api::screen::{
    AppRequest, HostInfo, JoinInfo, MenuInput, Phase, Places, Screen, ScreenContext, ScreenFacts, ScreenOutcome,
    SlotId, UiElement,
};
use pwc_mod_api::session::Session;
use pwc_mod_api::{Mod, ModRegistrar};
use pwc_ui_kit::{
    apply_text_op, draw_waiting, drive, gather, parse_port, present, Cursor, DefaultTheme, EditBuf, Framed, Intent,
    Menu, MenuTheme, Msg, Notice, PresentedView, Row, Style, View, PORT_ERROR,
};

/// The package entry point: installs [`StartScreenMod`].
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(StartScreenMod::new());
}

/// The start-screen mod (id `start`): answers the root screen slot.
pub struct StartScreenMod;

impl StartScreenMod {
    /// The default start screen.
    pub fn new() -> Self {
        Self
    }
}

impl Default for StartScreenMod {
    fn default() -> Self {
        Self::new()
    }
}

impl Mod for StartScreenMod {
    fn name(&self) -> &str {
        "Start"
    }

    fn id(&self) -> &'static str {
        "start"
    }

    fn root_screen(&self, facts: &ScreenFacts) -> Option<Box<dyn Screen>> {
        Some(Box::new(DefaultStart::open(facts)))
    }
}

/// The screens the player sees: main, worlds (the saved-world list), host, join.
pub struct DefaultStart {
    main: MainMenu,
    cursor: Cursor,
    overlay: Option<Overlay>,
}

enum Overlay {
    Worlds(Framed<WorldsMenu>),
    Host(Framed<HostMenu>),
    Join(Framed<JoinMenu>),
}

impl DefaultStart {
    /// The main menu, showing the notice the core opened it with.
    pub fn open(facts: &ScreenFacts) -> Self {
        Self { main: MainMenu::with_notice(facts.notice.map(str::to_string)), cursor: Cursor::default(), overlay: None }
    }

    /// The page on screen and its selected row.
    pub fn presented(&self, ctx: &ScreenContext) -> (PresentedView, usize) {
        let scale = ctx.settings().menu_scale;
        match &self.overlay {
            Some(Overlay::Worlds(frame)) => {
                let (view, sel) = frame.view_sel(ctx);
                (present(view, scale), sel)
            }
            Some(Overlay::Host(frame)) => {
                let (view, sel) = frame.view_sel(ctx);
                (present(view, scale), sel)
            }
            Some(Overlay::Join(frame)) => {
                let (view, sel) = frame.view_sel(ctx);
                (present(view, scale), sel)
            }
            None => {
                let view = self.main.view(&ctx.facts);
                let sel = self.cursor.resolved(&view);
                (present(view, scale), sel)
            }
        }
    }

    /// One frame of gathered intents (what [`Screen::update`] does with the core's input).
    pub fn update_intents(&mut self, intents: &[Intent], ctx: &mut ScreenContext) -> ScreenOutcome {
        if let Some(overlay) = &mut self.overlay {
            let outcome = match overlay {
                Overlay::Worlds(frame) => frame.update_intents(intents, ctx),
                Overlay::Host(frame) => frame.update_intents(intents, ctx),
                Overlay::Join(frame) => frame.update_intents(intents, ctx),
            };
            return match outcome {
                ScreenOutcome::Back => {
                    self.overlay = None;
                    ScreenOutcome::Stay
                }
                other => other,
            };
        }

        let view = self.main.view(&ctx.facts);
        // The rows can change between ticks (a save appears or vanishes), so settle the cursor on
        // a selectable row before the driver reads it.
        self.cursor.normalize(&view);
        match drive(intents, &view, &mut self.cursor) {
            Some(Msg::Pick(action)) => {
                self.main.notice = None;
                match action {
                    MainAction::NewWorld => ScreenOutcome::Request(AppRequest::NewWorld),
                    MainAction::Worlds => {
                        self.overlay = Some(Overlay::Worlds(Framed::new(WorldsMenu::default())));
                        ScreenOutcome::Stay
                    }
                    MainAction::Host => {
                        self.overlay = Some(Overlay::Host(Framed::new(HostMenu::new(ctx.facts.session))));
                        ScreenOutcome::Stay
                    }
                    MainAction::Join => {
                        self.overlay = Some(Overlay::Join(Framed::new(JoinMenu::new(ctx.facts.session))));
                        ScreenOutcome::Stay
                    }
                    MainAction::Entry(id) => ScreenOutcome::Open(id),
                    MainAction::Quit => ScreenOutcome::Request(AppRequest::Quit),
                }
            }
            Some(Msg::Back) => {
                self.main.notice = None;
                ScreenOutcome::Stay
            }
            _ => ScreenOutcome::Stay,
        }
    }
}

impl Screen for DefaultStart {
    fn update(&mut self, input: &MenuInput, ctx: &mut ScreenContext) -> ScreenOutcome {
        let intents = gather(input);
        self.update_intents(&intents, ctx)
    }

    fn draw(&self, ctx: &ScreenContext, out: &mut Vec<UiElement>, size: (i32, i32)) {
        if ctx.facts.phase != Phase::Idle {
            draw_waiting(out, ctx.facts.phase, size);
            return;
        }
        let (page, sel) = self.presented(ctx);
        DefaultTheme.draw(out, page, sel, size.0, size.1);
    }
}

/// The start menu: New World, Worlds (the saved-world page), Host, Join, the main-menu entries of
/// other packages, Quit. Saves are not listed inline so the menu stays short.
struct MainMenu {
    notice: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainAction {
    NewWorld,
    Worlds,
    Host,
    Join,
    /// A registered entry, by id.
    Entry(&'static str),
    Quit,
}

impl MainMenu {
    fn with_notice(notice: Option<String>) -> Self {
        Self { notice }
    }

    fn view(&self, facts: &ScreenFacts) -> View<MainAction> {
        let worlds_detail = match facts.saves.len() {
            0 => "none saved yet".to_string(),
            1 => "1 saved".to_string(),
            n => format!("{n} saved"),
        };
        let mut rows = vec![
            Row::action("New World", MainAction::NewWorld),
            Row::action("Worlds", MainAction::Worlds).detail(worlds_detail),
            Row::action("Host Server", MainAction::Host),
            Row::action("Join Server", MainAction::Join),
        ];
        rows.extend(facts.entries_for(Places::MAIN).map(|entry| Row::action(entry.label, MainAction::Entry(entry.id))));
        rows.push(Row::action("Quit", MainAction::Quit));
        View {
            title: "PROJECT WATT CUBED".to_string(),
            style: Style::Title { subtitle: "an infinite voxel world of elements".to_string() },
            rows,
            default: None,
            hint: "Up/Down select   Enter choose".to_string(),
            notice: self.notice.as_ref().map(|text| Notice::info(text.clone())),
        }
    }
}

#[derive(Clone, Copy)]
enum WorldsAction {
    Load(usize),
    /// The confirm row of a world D was pressed on.
    Delete(usize),
    Back,
}

/// The worlds page: one row per saved world (newest first, as the save store
/// lists them), then Back. Enter loads; Esc goes back to the main menu. D on a
/// world asks to delete it: a short confirm line appears on that row, Enter
/// moves the world to the trash, and any other key cancels.
#[derive(Default)]
struct WorldsMenu {
    /// The world D was pressed on, awaiting Enter.
    confirm: Option<SlotId>,
}

impl Menu for WorldsMenu {
    type Action = WorldsAction;

    fn view(&self, ctx: &ScreenContext) -> View<WorldsAction> {
        let saves = ctx.facts.saves;
        let mut rows: Vec<Row<WorldsAction>> = Vec::with_capacity(saves.len() + 1);
        if saves.is_empty() {
            rows.push(Row::heading("No saved worlds yet — pick New World on the main menu"));
        }
        for (i, slot) in saves.iter().enumerate() {
            if self.confirm.as_ref() == Some(&slot.id) {
                rows.push(
                    Row::action(format!("Delete {}?", slot.id), WorldsAction::Delete(i))
                        .detail("Enter to delete · any other key cancels"),
                );
                continue;
            }
            let row = match &slot.meta {
                Ok(meta) => Row::action(format!("Load: {}", meta.name), WorldsAction::Load(i))
                    .detail(format!("{} · {} edits", fmt_playtime(meta.playtime_secs), meta.edit_count)),
                Err(_) => Row::action(format!("Load: {} (damaged)", slot.id), WorldsAction::Load(i))
                    .detail("unreadable — a backup may still load"),
            };
            rows.push(row);
        }
        rows.push(Row::action("Back", WorldsAction::Back));
        View {
            title: "WORLDS".to_string(),
            style: Style::Panel,
            rows,
            default: None,
            hint: "Up/Down select   Enter load   D delete   Esc back".to_string(),
            notice: None,
        }
    }

    fn update(&mut self, msg: Msg<WorldsAction>, ctx: &mut ScreenContext) -> ScreenOutcome {
        let confirm = self.confirm.take();
        let saves = ctx.facts.saves;
        match msg {
            Msg::Key(WorldsAction::Load(i) | WorldsAction::Delete(i), 'd' | 'D') => {
                self.confirm = saves.get(i).map(|slot| slot.id.clone());
                ScreenOutcome::Stay
            }
            Msg::Pick(WorldsAction::Delete(i)) => match saves.get(i) {
                Some(slot) => ScreenOutcome::Request(AppRequest::Delete(slot.id.clone())),
                None => ScreenOutcome::Stay,
            },
            Msg::Pick(WorldsAction::Load(i)) => match saves.get(i) {
                Some(slot) => ScreenOutcome::Request(AppRequest::Load(slot.id.clone())),
                None => ScreenOutcome::Stay,
            },
            // Esc first cancels a pending delete, then leaves the page.
            Msg::Back if confirm.is_some() => ScreenOutcome::Stay,
            Msg::Pick(WorldsAction::Back) | Msg::Back => ScreenOutcome::Back,
            _ => ScreenOutcome::Stay,
        }
    }
}

/// Play time as whole hours and minutes, e.g. `1h 1m played`; spare seconds are dropped and hours
/// keep counting past a day.
fn fmt_playtime(secs: u64) -> String {
    const SECS_PER_MIN: u64 = 60;
    const SECS_PER_HOUR: u64 = 60 * SECS_PER_MIN;
    let h = secs / SECS_PER_HOUR;
    let m = (secs % SECS_PER_HOUR) / SECS_PER_MIN;
    if h > 0 { format!("{h}h {m}m played") } else { format!("{m}m played") }
}

#[derive(Clone, Copy)]
enum ConnectionAction {
    Address,
    Port,
    Password,
    Name,
    Submit,
}

/// Shared host/join form; the mode supplies only the extra address row and the
/// final request while editing, validation, and common fields stay identical.
struct ConnectionMenu<const JOIN: bool> {
    address: EditBuf,
    port: EditBuf,
    password: EditBuf,
    name: EditBuf,
    /// Why the last submit was refused; cleared by the next edit.
    rejection: Option<&'static str>,
}

type HostMenu = ConnectionMenu<false>;
type JoinMenu = ConnectionMenu<true>;

impl<const JOIN: bool> ConnectionMenu<JOIN> {
    fn new(session: &Session) -> Self {
        // Byte caps per field: a host name or IP literal, a port number, a password.
        const ADDRESS_CAP: usize = 64;
        const PORT_CAP: usize = 5;
        const PASSWORD_CAP: usize = 64;
        Self {
            address: remembered_or(&session.address, "127.0.0.1", ADDRESS_CAP),
            port: remembered_or(&session.port, &DEFAULT_PORT.to_string(), PORT_CAP),
            password: EditBuf::new(PASSWORD_CAP),
            name: remembered_or(&session.name, "player", MAX_NAME),
            rejection: None,
        }
    }
}

impl<const JOIN: bool> Menu for ConnectionMenu<JOIN> {
    type Action = ConnectionAction;

    fn view(&self, _ctx: &ScreenContext) -> View<ConnectionAction> {
        let (title, password, submit, hint) = if JOIN {
            ("JOIN SERVER", "Password", "Connect", "type to edit   Enter connect   Esc back")
        } else {
            ("HOST SERVER", "Password (optional)", "Start", "type to edit   Enter start   Esc back")
        };
        let mut rows = Vec::new();
        if JOIN {
            rows.push(text_row("Address", &self.address, false, ConnectionAction::Address));
        }
        rows.extend([
            text_row("Port", &self.port, false, ConnectionAction::Port),
            text_row(password, &self.password, true, ConnectionAction::Password),
            text_row("Your name", &self.name, false, ConnectionAction::Name),
            Row::action(submit, ConnectionAction::Submit),
        ]);
        View {
            title: title.to_string(),
            style: Style::Panel,
            rows,
            default: Some(ConnectionAction::Submit),
            hint: hint.to_string(),
            notice: self.rejection.map(|why| Notice::error(why.to_string())),
        }
    }

    fn update(&mut self, msg: Msg<ConnectionAction>, _ctx: &mut ScreenContext) -> ScreenOutcome {
        match msg {
            Msg::Edited(field, op) => {
                // Any change to the form retracts a refusal about its previous contents.
                self.rejection = None;
                match field {
                    ConnectionAction::Address => apply_text_op(&mut self.address, op),
                    ConnectionAction::Port => apply_text_op(&mut self.port, op),
                    ConnectionAction::Password => apply_text_op(&mut self.password, op),
                    ConnectionAction::Name => apply_text_op(&mut self.name, op),
                    ConnectionAction::Submit => {}
                }
                ScreenOutcome::Stay
            }
            Msg::Pick(ConnectionAction::Submit) => match parse_port(self.port.text(), DEFAULT_PORT) {
                Some(port) => {
                    let password = self.password.text().to_string();
                    let name = self.name.text().to_string();
                    let request = if JOIN {
                        AppRequest::Join(JoinInfo { host: self.address.text().trim().to_string(), port, password, name })
                    } else {
                        AppRequest::Host(HostInfo { port, password, name })
                    };
                    ScreenOutcome::Request(request)
                }
                None => {
                    self.rejection = Some(PORT_ERROR);
                    ScreenOutcome::Stay
                }
            },
            Msg::Back => ScreenOutcome::Back,
            _ => ScreenOutcome::Stay,
        }
    }
}

/// A field buffer prefilled with the remembered text, or with `fallback` when nothing was
/// remembered (an empty string). A remembered value is used as is, untrimmed.
fn remembered_or(saved: &str, fallback: &str, cap: usize) -> EditBuf {
    let init = if saved.is_empty() { fallback } else { saved };
    EditBuf::with(init, cap)
}

/// An editable form row showing `buf`, with the caret counted in characters so it lands correctly
/// after multi-byte text.
fn text_row<A: Copy>(label: &str, buf: &EditBuf, masked: bool, tag: A) -> Row<A> {
    Row::text(label, buf.text().to_string(), buf.caret_chars(), masked, tag)
}

#[cfg(test)]
mod tests;
