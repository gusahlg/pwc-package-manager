//! Who hears whom. Hold V to send opus frames on the `"voice"` channel. A peer
//! is audible while they are in the interest set; leaving the roster closes the
//! session. Distance is the audio kernel's job. The microphone stays shut unless
//! this mod is on, voice is enabled, and the talk key is held on a live connection.

use pwc_mod_api::audio::{AudioApi, AudioView, CapturedFrame, GameEvent, ModFrame, ModLink};
use pwc_mod_api::engine::Key;
use pwc_mod_api::input::intent::Chord;
use pwc_mod_api::{Action, Mod, ModRegistrar, ESSENTIALS};

const CHANNEL: &str = "voice";
const TALK: &str = "voice.talk";

/// The package entry point.
pub fn register(registrar: &mut ModRegistrar) {
    registrar.add(ProximityChat::default());
}

/// The proximity-chat mod (id `proximity_chat`).
#[derive(Default)]
pub struct ProximityChat {
    /// Peers we have positioned. Closed when they leave the roster, not when they go invisible.
    known: Vec<u32>,
    /// Encoded frames a test queued. Non-empty means "send these, do not open the microphone".
    outbox: Vec<Vec<u8>>,
    inbox: Vec<ModFrame>,
    captured: Vec<CapturedFrame>,
    seq: u32,
}

impl ProximityChat {
    /// Queue one encoded frame. The next in-world frame sends it and leaves the microphone shut.
    pub fn inject(&mut self, bytes: Vec<u8>) {
        self.outbox.push(bytes);
    }

    fn reset(&mut self, audio: &mut AudioApi) {
        audio.stop_capture();
        for id in self.known.drain(..) {
            audio.close_voice(id);
        }
        self.outbox.clear();
        self.inbox.clear();
        self.captured.clear();
        self.seq = 0;
    }

    fn place(&mut self, view: &AudioView, audio: &mut AudioApi) {
        if !view.peers.is_empty() {
            for peer in view.peers {
                audio.position_voice(peer.id, peer.at, peer.visible);
            }
        }
        if !self.known.is_empty() {
            let peers = view.peers;
            self.known.retain(|id| {
                let present = peers.iter().any(|peer| peer.id == *id);
                if !present {
                    audio.close_voice(*id);
                }
                present
            });
        }
        for peer in view.peers {
            if !self.known.iter().any(|id| *id == peer.id) {
                self.known.push(peer.id);
            }
        }
    }

    fn hear(&mut self, view: &AudioView, audio: &mut AudioApi, link: &mut ModLink) {
        if !view.in_world || !link.connected() || !link.pending(CHANNEL) {
            return;
        }
        link.drain(CHANNEL, &mut self.inbox);
        for frame in self.inbox.drain(..) {
            audio.push_voice(frame.sender, frame.seq, &frame.bytes);
        }
    }

    fn talk(&mut self, view: &AudioView, audio: &mut AudioApi, link: &mut ModLink) {
        if !view.in_world {
            return;
        }
        let online = link.connected();
        if online && !self.outbox.is_empty() {
            for bytes in self.outbox.drain(..) {
                let seq = self.seq;
                self.seq = self.seq.wrapping_add(1);
                let _ = link.send(CHANNEL, seq, &bytes);
            }
            return;
        }
        if online && view.voice_enabled && view.action(TALK) {
            audio.start_capture();
            audio.drain_capture(&mut self.captured);
            for frame in self.captured.drain(..) {
                let _ = link.send(CHANNEL, frame.seq, &frame.bytes);
            }
            return;
        }
        audio.stop_capture();
    }
}

impl Mod for ProximityChat {
    fn name(&self) -> &str {
        "Proximity chat"
    }

    fn id(&self) -> &'static str {
        "proximity_chat"
    }

    fn description(&self) -> &str {
        "Push-to-talk voice. People nearby hear you; the mixer does the distance."
    }

    fn group(&self) -> &'static str {
        ESSENTIALS
    }

    fn actions(&self) -> &[Action] {
        const CHORDS: &[Chord] = &[Chord::key(Key::V)];
        const ACTIONS: &[Action] = &[Action {
            id: TALK,
            label: "Push to talk",
            default: CHORDS,
            repeat: false,
            held: true,
        }];
        ACTIONS
    }

    fn on_game_event(&mut self, ev: &GameEvent, audio: &mut AudioApi) {
        match ev {
            GameEvent::EnterWorld | GameEvent::LeaveWorld => self.reset(audio),
            _ => {}
        }
    }

    fn on_audio(&mut self, view: &AudioView, audio: &mut AudioApi, link: &mut ModLink) {
        self.place(view, audio);
        self.hear(view, audio, link);
        self.talk(view, audio, link);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwc_mod_api::audio::{AudioBench, PeerAudio};
    use pwc_mod_api::engine::DVec3;
    use pwc_mod_api::net::ChannelPair;
    use pwc_mod_api::ActionSet;

    const IDS: &[&str] = &[TALK];

    fn view<'a>(peers: &'a [PeerAudio], enabled: bool, talking: bool) -> AudioView<'a> {
        let mut actions = ActionSet::NONE;
        if talking {
            actions.insert(0);
        }
        AudioView {
            dt: 1.0 / 60.0,
            pos: DVec3::ZERO,
            peers,
            in_world: true,
            voice_enabled: enabled,
            hear_voice: true,
            actions,
            ids: IDS,
        }
    }

    fn frame(chat: &mut ProximityChat, bench: &mut AudioBench, view: &AudioView, link: &mut ModLink) {
        let mut api = bench.api(None);
        chat.on_audio(view, &mut api, link);
    }

    #[test]
    fn a_disabled_or_offline_voice_does_not_open_the_microphone() {
        let mut chat = ProximityChat::default();
        let mut bench = AudioBench::recording();
        let seen = view(&[], false, true);
        let mut link = ModLink::idle();
        frame(&mut chat, &mut bench, &seen, &mut link);
        assert!(!bench.capture_requested(), "voice disabled never asks for the microphone");
        let offline = view(&[], true, true);
        frame(&mut chat, &mut bench, &offline, &mut link);
        assert!(!bench.capture_requested(), "no connection, no microphone");
    }

    #[test]
    fn two_clients_send_and_play_voice_over_the_channel() {
        let mut pair = ChannelPair::open();
        let mut sender = ProximityChat::default();
        let mut receiver = ProximityChat::default();
        sender.inject(vec![1, 2, 3, 4]);
        let mut send_bench = AudioBench::recording();
        let mut recv_bench = AudioBench::recording();
        let talking = view(&[], true, true);
        pair.with_a(|mut link| frame(&mut sender, &mut send_bench, &talking, &mut link));
        assert!(!send_bench.capture_requested(), "an injected frame does not open the microphone");

        let mut arrived = false;
        for _ in 0..40 {
            pair.pump();
            arrived = pair.with_b(|link| link.pending(CHANNEL));
            if arrived {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(arrived, "the voice channel did not deliver");

        let peers = [PeerAudio {
            id: pair.id_a(),
            at: DVec3::new(8.0, 40.0, 8.0),
            feet: DVec3::new(8.0, 38.4, 8.0),
            visible: true,
            gait: 0.0,
            speed: 0.0,
        }];
        let hearing = view(&peers, true, false);
        pair.with_b(|mut link| frame(&mut receiver, &mut recv_bench, &hearing, &mut link));
        assert!(recv_bench.stream_count() >= 1, "the received frame opened a voice session");
        assert!(!recv_bench.capture_requested());
    }
}
