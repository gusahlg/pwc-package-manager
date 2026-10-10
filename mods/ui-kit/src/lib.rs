//! The PWC UI kit: the menu framework, the default look and the text widgets every menu package
//! draws with. A library: it registers nothing and holds no state, so packages that use it
//! depend on it and on nothing else of each other's.
//!
//! - [`menu`]: a [`Menu`] builds a [`View`] of rows and answers one [`Msg`] at a time; [`Framed`]
//!   adds the cursor and makes it a core [`Screen`](pwc_mod_api::screen::Screen).
//! - [`theme`]: lays a view out and turns it into the core's [`UiElement`]s; [`draw_waiting`] is the
//!   connecting and loading page.
//! - [`text`]: [`TextInput`], [`EditBuf`], [`Ring`], [`Completion`], [`common_prefix`].
//! - [`testing`]: a [`Fixture`](testing::Fixture) that builds a screen context for tests.
//!
//! [`UiElement`]: pwc_mod_api::screen::UiElement

pub mod menu;
pub mod testing;
pub mod text;
pub mod theme;

pub use menu::{
    apply_text_op, drive, gather, parse_port, Cursor, Dir, Framed, Intent, Level, Menu, Msg, Notice, Row, RowKind,
    Style, TextOp, ValueView, View, PORT_ERROR,
};
pub use text::{common_prefix, Completion, EditBuf, Ring, TextInput};
pub use theme::{
    draw_waiting, label, present, shadowed, DefaultTheme, MenuTheme, PresentedRow, PresentedView, RowRect, MENU_BG,
    MENU_DIM,
};
