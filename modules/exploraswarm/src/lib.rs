// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Exploraswarm: neural-network players that join a server and explore it.
//!
//! The command is `swarm`. Learning machinery and rule sets are plain Rust; only this file and
//! `client.rs` talk to the game.

use pwc_module_api::{Cli, ModuleRegistrar};

mod actions;
mod agent;
mod archive;
mod body;
mod cli;
mod client;
mod discover;
mod emitter;
mod experience;
mod facts;
mod nn;
mod rng;
mod rules;
mod senses;
mod store;
mod swarm;

#[cfg(test)]
mod testing;

/// Package entry. Registers the `swarm` command.
pub fn register(registrar: &mut ModuleRegistrar) {
    registrar.cli(Cli {
        name: "swarm",
        about: "Learning headless players that explore a multiplayer server.",
        run: client::entry,
    });
}
