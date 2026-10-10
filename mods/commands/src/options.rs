//! Options: `/set`, `/time` and the audio commands. Each edits the value it is handed; the core
//! applies and saves changed settings, re-mixes the audio and plays the voice test cue.

use pwc_mod_api::audio::GameEvent;
use pwc_mod_api::settings::options::CORE;
use pwc_mod_api::settings::{Applies, OptionsRef, Settings};
use pwc_mod_api::sky::{DayLength, Sky};
use pwc_mod_api::ui::Line;
use pwc_mod_api::{GameContext, VisualMask};

use crate::{rejected, shown};

/// Appended to a lane no installed, unsuspended package provides a visual group for.
pub const UNAVAILABLE: &str = "(unavailable in this build)";

/// `/time` — show or set the day/night clock, or change the cycle length.
///
///   `time`                 show the current time and cycle length
///   `time set <when>`      `0..1` fraction, `0..24` hour, or a name
///                          (dawn/day/noon/dusk/night/midnight)
///   `time length <secs>`   set how long a full cycle lasts
pub(crate) fn time(args: &[&str], sky: &mut Sky) -> Vec<Line> {
    match args {
        [] => shown(vec![format!(
            "time: {}  ({:.3} of day, cycle {:.0}s)",
            clock_label(sky.clock.day()),
            sky.clock.day(),
            sky.day_length.0,
        )]),
        ["set", when] => match parse_when(when) {
            Some(day) => {
                sky.clock.set_day(day);
                shown(vec![format!("time set to {}", clock_label(day))])
            }
            None => rejected(vec!["/time: use 0..1, 0..24, or dawn|day|noon|dusk|night".to_string()]),
        },
        ["length", secs] => match secs.parse::<f64>() {
            Ok(s) if s.is_finite() => {
                sky.day_length = DayLength::clamped(s);
                shown(vec![format!("day length set to {:.0}s", sky.day_length.0)])
            }
            _ => rejected(vec!["/time: length must be a number of seconds".to_string()]),
        },
        _ => rejected(vec!["usage: /time [set <when> | length <secs>]".to_string()]),
    }
}

/// Parse a `/time set` argument into a day fraction in `[0, 1)`. Accepts named
/// times, a `0..1` fraction, or a `0..24` hour.
pub(crate) fn parse_when(s: &str) -> Option<f64> {
    let named = match s.to_ascii_lowercase().as_str() {
        "midnight" => Some(0.0),
        "dawn" | "sunrise" => Some(0.25),
        "morning" => Some(0.35),
        "day" | "noon" | "midday" => Some(0.5),
        "dusk" | "sunset" => Some(0.75),
        "night" => Some(0.9),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let v = s.parse::<f64>().ok().filter(|v| v.is_finite())?;
    // <= 1 reads as a fraction; otherwise as an hour of a 24-hour day.
    Some(if v <= 1.0 { v.rem_euclid(1.0) } else { (v / 24.0).rem_euclid(1.0) })
}

/// A short `HH:MM`-ish label for a day fraction (0.0 = 00:00, 0.5 = 12:00).
fn clock_label(day: f64) -> String {
    let total = (day.rem_euclid(1.0) * 24.0 * 60.0).round() as i32;
    format!("{:02}:{:02}", (total / 60) % 24, total % 60)
}


/// One entry of the options view as `/set` shows it: `key = value`, then "(next new world)" for a
/// value that applies to the next new world and the unavailable marker for a lane no installed
/// package provides.
fn entry_line(options: &OptionsRef, i: usize, visuals: VisualMask) -> String {
    let info = options.info(i);
    let mut line = format!("{} = {}", options.full_key(i), options.show(i));
    if info.applies == Applies::NextWorld {
        line.push_str(" (next new world)");
    }
    if info.owner == CORE && visuals.strips(info.key) {
        line.push(' ');
        line.push_str(UNAVAILABLE);
    }
    line
}

/// `/set [key [value]]` — every tunable through the game's options registry: the core's settings
/// (by key or alias: `msaa`, `fps`) and every package's options (by `<package>.<key>`).
///
///   `set`               list every key with its value
///   `set <key>`         show one
///   `set <key> <value>` change one (parsed and clamped; a bad value changes nothing)
///
/// The core applies and saves what changed, and the packages hear their options.
pub(crate) fn set(args: &[&str], game: &mut GameContext) -> Vec<Line> {
    let visuals = game.visuals;
    match args {
        [] => {
            let options = game.options();
            let mut lines = vec!["settings (/set <key> <value>):".to_string()];
            lines.extend((0..options.len()).map(|i| format!("  {}", entry_line(&options, i, visuals))));
            shown(lines)
        }
        [key, value @ ..] => {
            let Some(i) = game.options().find(key) else {
                return rejected(vec![format!("unknown setting '{key}' - type '/set' to list them")]);
            };
            if !value.is_empty() && !game.options_mut().parse(i, &value.join(" ")) {
                let options = game.options();
                return rejected(vec![format!("usage: /set {} {}", options.full_key(i), options.hint(i))]);
            }
            shown(vec![entry_line(&game.options(), i, visuals)])
        }
    }
}

/// `/mute` — toggle the transient master mute. Not persisted (resets each launch);
/// the core pushes the mutated [`Settings`] to the mixer.
pub(crate) fn mute(settings: &mut Settings) -> Vec<Line> {
    settings.muted = !settings.muted;
    shown(vec![format!("audio {}", if settings.muted { "muted" } else { "unmuted" })])
}

/// `/deafen` — toggle whether incoming voice is heard. Flips the persisted
/// `voice_incoming` gate (deafen is its inverse), so the core saves the change.
pub(crate) fn deafen(settings: &mut Settings) -> Vec<Line> {
    settings.voice_incoming = !settings.voice_incoming;
    let msg = if settings.voice_incoming { "undeafened (hearing voice)" } else { "deafened (voice muted)" };
    shown(vec![msg.to_string()])
}

/// `/audio <master|effects|voice> <0-100>` — set one mix volume, clamped to 0..=100.
/// The core persists the mutated [`Settings`]; a bad channel or value changes nothing.
pub(crate) fn audio(args: &[&str], game: &mut GameContext) -> Vec<Line> {
    let usage = || rejected(vec!["usage: /audio <master|effects|voice> <0-100>".to_string()]);
    let [channel, value] = args else {
        return usage();
    };
    let Ok(pct) = value.parse::<u8>() else {
        return usage();
    };
    if !matches!(*channel, "master" | "effects" | "sfx" | "voice") {
        return usage();
    }
    let settings = game.settings_mut();
    let field = match *channel {
        "master" => &mut settings.master_volume,
        "effects" | "sfx" => &mut settings.effects_volume,
        _ => &mut settings.voice_volume,
    };
    *field = pct.min(100);
    shown(vec![format!("{channel} volume {}%", *field)])
}

/// `/voicetest` — ask the core to play the local test cue, so the user can check their voice path.
pub(crate) fn voicetest(game: &mut GameContext) -> Vec<Line> {
    game.events.push(GameEvent::VoiceTest);
    shown(vec!["queued a voice test cue".to_string()])
}
