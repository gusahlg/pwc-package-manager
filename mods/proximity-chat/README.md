# Proximity chat (`pwc.proximity-chat`)

Push-to-talk voice. Hold V and people who can see you hear you. The audio kernel does the
distance and the occlusion; this mod decides who is audible and when the microphone is open.

Mod id: `proximity_chat` (display name "Proximity chat"), in the Essentials group, enabled by
default. With this mod off the microphone is never opened, and voice is neither sent nor played.

## How it works

Encoded frames travel on the mod channel `"voice"`. The server stamps the sender and relays a
frame only to players who can see them. A peer you can see has a voice session at their position.
A peer who drops out of the roster is closed. Going out of sight keeps the session and silences it.

The microphone opens only while all of these are true: you are in a world, voice is enabled in
settings, V is held, and the connection is up. Releasing V stops transmit. Hearing does not
require you to be talking, and it does not require voice transmit to be enabled. Deafening (the
"hear voice" setting) is a core mix flag: frames are still delivered, and the mixer mutes them.

## Controls

| Input | Action |
|---|---|
| `V` (hold) | Push to talk (`voice.talk`) |

## Settings

None of its own. Master, effects, voice volume, voice enabled and hear voice stay core settings.
`/mute`, `/deafen`, `/audio` and `/voicetest` stay on the Developer Toolkit. `/voicetest` plays a
cue through the sounds mod; it does not open the microphone.

## Persisted state

Nothing besides `proximity_chat=on|off` in `mods.cfg`.

## Dependencies

The PWC mod API (`pwc-api ^2.1`).

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
