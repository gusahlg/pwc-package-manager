//! A screen context for tests: settings, options and facts with sensible empty defaults, every
//! field public so a test sets what it needs.

use pwc_mod_api::BuildInfo;
use pwc_mod_api::screen::{Phase, ScreenContext, ScreenEntry, ScreenFacts, Slot, VERSION};
use pwc_mod_api::session::Session;
use pwc_mod_api::settings::{Options, Settings};
use pwc_mod_api::VisualMask;

/// Everything a [`ScreenContext`] borrows.
pub struct Fixture {
    pub settings: Settings,
    pub options: Options,
    pub saves: Vec<Slot>,
    pub session: Session,
    pub hosting: bool,
    pub notice: Option<String>,
    pub phase: Phase,
    pub in_world: bool,
    pub build: BuildInfo,
    pub suspended: Vec<String>,
    pub entries: Vec<ScreenEntry>,
    pub visuals: VisualMask,
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture {
    /// Default settings, no options, no saves, idle, out of a world, an empty build, every visual
    /// group provided.
    pub fn new() -> Self {
        Self {
            settings: Settings::default(),
            options: Options::new(),
            saves: Vec::new(),
            session: Session::default(),
            hosting: false,
            notice: None,
            phase: Phase::Idle,
            in_world: false,
            build: BuildInfo::EMPTY,
            suspended: Vec::new(),
            entries: Vec::new(),
            visuals: VisualMask::ALL,
        }
    }

    /// The facts these fields describe.
    pub fn facts(&self) -> ScreenFacts<'_> {
        ScreenFacts {
            saves: &self.saves,
            session: &self.session,
            version: VERSION,
            hosting: self.hosting,
            notice: self.notice.as_deref(),
            phase: self.phase,
            in_world: self.in_world,
            build: &self.build,
            suspended: &self.suspended,
            entries: &self.entries,
            visuals: self.visuals,
        }
    }

    /// A context over these fields.
    pub fn ctx(&mut self) -> ScreenContext<'_> {
        let Self { settings, options, saves, session, hosting, notice, phase, in_world, build, suspended, entries, visuals } =
            self;
        let facts = ScreenFacts {
            saves,
            session,
            version: VERSION,
            hosting: *hosting,
            notice: notice.as_deref(),
            phase: *phase,
            in_world: *in_world,
            build,
            suspended,
            entries,
            visuals: *visuals,
        };
        ScreenContext::new(facts, settings, options)
    }
}
