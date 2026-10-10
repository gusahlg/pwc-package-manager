//! Which cue plays for each game fact. The audio service carries the cue out;
//! this mod only chooses the name. A class cue (`break_stone`) wins when the
//! catalog has it; otherwise the `*_default` cue plays. Gain is 1.

use pwc_mod_api::audio::{AudioApi, GameEvent};
use pwc_mod_api::block::SoundClass;
use pwc_mod_api::engine::DVec3;
use pwc_mod_api::{Mod, ModRegistrar};

/// The package entry point.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(Sounds);
}

/// The sounds mod (id `sounds`).
pub struct Sounds;

impl Mod for Sounds {
    fn name(&self) -> &str {
        "Sounds"
    }

    fn id(&self) -> &'static str {
        "sounds"
    }

    fn on_game_event(&mut self, ev: &GameEvent, audio: &mut AudioApi) {
        match *ev {
            GameEvent::BlockBroken { at, block, .. } | GameEvent::ToolReacted { at, block } => {
                let class = audio.block_sound(block).class;
                play_class(audio, Kind::Break, class, at);
            }
            GameEvent::BlockPlaced { at, block, .. } => {
                let class = audio.block_sound(block).class;
                play_class(audio, Kind::Place, class, at);
            }
            GameEvent::Footstep { at, ground } => {
                let class = audio.block_sound(ground).class;
                play_class(audio, Kind::Step, class, at);
            }
            GameEvent::PeerSwing { at, .. } => {
                audio.play_at("swing", at, 1.0);
            }
            GameEvent::UiNavigate | GameEvent::UiConfirm => {
                audio.play_ui("menu_click", 1.0);
            }
            GameEvent::VoiceTest => {
                audio.play_ui("voicetest", 1.0);
            }
            GameEvent::Swing { .. } | GameEvent::EnterWorld | GameEvent::LeaveWorld => {}
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Break,
    Place,
    Step,
}

fn play_class(audio: &mut AudioApi, kind: Kind, class: SoundClass, at: DVec3) {
    let name = specific(kind, class);
    if audio.play_at(name, at, 1.0) {
        return;
    }
    let fallback = fallback(kind);
    if name != fallback {
        audio.play_at(fallback, at, 1.0);
    }
}

macro_rules! by_class {
    ($class:expr, $stem:literal) => {
        match $class {
            SoundClass::Stone => concat!($stem, "_stone"),
            SoundClass::Soil => concat!($stem, "_soil"),
            SoundClass::Wood => concat!($stem, "_wood"),
            SoundClass::Glass => concat!($stem, "_glass"),
            SoundClass::Foliage => concat!($stem, "_foliage"),
            SoundClass::Open => concat!($stem, "_open"),
        }
    };
}

fn specific(kind: Kind, class: SoundClass) -> &'static str {
    match kind {
        Kind::Break => by_class!(class, "break"),
        Kind::Place => by_class!(class, "place"),
        Kind::Step => by_class!(class, "step"),
    }
}

fn fallback(kind: Kind) -> &'static str {
    match kind {
        Kind::Break => "break_default",
        Kind::Place => "place_default",
        Kind::Step => "step_default",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::audio::{AudioBench, Play};
    use pwc_mod_api::block::AIR;
    use pwc_mod_api::world::World;

    #[test]
    fn class_cues_fall_back_to_the_default_name() {
        assert_eq!(specific(Kind::Break, SoundClass::Stone), "break_stone");
        assert_eq!(specific(Kind::Place, SoundClass::Wood), "place_wood");
        assert_eq!(specific(Kind::Step, SoundClass::Open), "step_open");
        assert_eq!(specific(Kind::Step, SoundClass::Glass), "step_glass");
        assert_eq!(fallback(Kind::Break), "break_default");
        assert_eq!(fallback(Kind::Place), "place_default");
        assert_eq!(fallback(Kind::Step), "step_default");
        for class in [
            SoundClass::Stone,
            SoundClass::Soil,
            SoundClass::Wood,
            SoundClass::Glass,
            SoundClass::Foliage,
            SoundClass::Open,
        ] {
            assert!(specific(Kind::Break, class).ends_with(class.as_str()));
            assert_ne!(specific(Kind::Break, class), fallback(Kind::Break));
        }
    }

    fn fire(bench: &mut AudioBench, world: &World, sounds: &mut Sounds, ev: GameEvent) {
        let mut api = bench.api(Some(world));
        sounds.on_game_event(&ev, &mut api);
    }

    fn names(plays: &[Play]) -> Vec<&str> {
        plays.iter().map(|play| play.cue.as_str()).collect()
    }

    /// The recording catalog has the six default cues and no class override, so every
    /// block lands on `*_default`, the same cue the old director played.
    #[test]
    fn events_play_the_same_cues_as_the_old_director() {
        let world = World::new(1);
        let mut bench = AudioBench::recording();
        let mut sounds = Sounds;
        let at = DVec3::new(1.0, 2.0, 3.0);
        fire(&mut bench, &world, &mut sounds, GameEvent::BlockBroken { at, block: AIR, local: true });
        fire(&mut bench, &world, &mut sounds, GameEvent::BlockBroken { at, block: AIR, local: false });
        fire(&mut bench, &world, &mut sounds, GameEvent::BlockPlaced { at, block: AIR, local: true });
        fire(&mut bench, &world, &mut sounds, GameEvent::ToolReacted { at, block: AIR });
        fire(&mut bench, &world, &mut sounds, GameEvent::Footstep { at, ground: AIR });
        fire(&mut bench, &world, &mut sounds, GameEvent::PeerSwing { at, peer: 4 });
        fire(&mut bench, &world, &mut sounds, GameEvent::Swing { at });
        fire(&mut bench, &world, &mut sounds, GameEvent::UiNavigate);
        fire(&mut bench, &world, &mut sounds, GameEvent::UiConfirm);
        fire(&mut bench, &world, &mut sounds, GameEvent::VoiceTest);
        fire(&mut bench, &world, &mut sounds, GameEvent::EnterWorld);
        fire(&mut bench, &world, &mut sounds, GameEvent::LeaveWorld);
        assert_eq!(
            names(bench.plays()),
            [
                "break_default",
                "break_default",
                "place_default",
                "break_default",
                "step_default",
                "swing",
                "menu_click",
                "menu_click",
                "voicetest",
            ]
        );
        assert!(bench.plays().iter().all(|play| play.gain == 1.0));
        assert_eq!(bench.clip_count(), 3, "ui cues play at once");
        bench.finish(&world);
        assert_eq!(bench.clip_count(), 9, "the six world cues join the three ui cues");
    }
}
