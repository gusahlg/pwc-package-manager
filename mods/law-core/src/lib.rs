//! The shared ground of the candidate physics packages. Every candidate law reads the same thing:
//! a box of real matter (configurations of the game's elements), and derives every constant it
//! needs from the one quantity the universe already has, the fit between elements. No candidate
//! names a material.
//!
//! - [`grid`]: the box and its six face directions.
//! - [`matter`]: a palette of configurations with cached readings and the bond table.
//! - [`candidate`]: the interface the physics lab and the experiments drive, and its ledger.
//! - [`search`]: deterministic helpers that find configurations with wanted readings.
//! - [`show`]: an ASCII slice of a candidate's state, for experiments and reports.

use pwc_mod_api::material;

pub mod candidate;
pub mod grid;
pub mod matter;
pub mod search;
pub mod show;

pub use candidate::{Candidate, Entry, Ledger};
pub use grid::{Grid, DIRS};
pub use matter::{Mat, Matter, Reading, VOID};

#[cfg(test)]
mod tests;
