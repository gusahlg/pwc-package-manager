# Sounds (`pwc.sounds`)

Footsteps, the sound of a block breaking or being placed, a tool reaction, another player's swing,
and the menu click. The voice-test cue (`/voicetest`) lives here too.

Mod id: `sounds`. Part of `pwc.essentials`. A build without this package plays nothing: the audio
device stays in the core, and the core itself starts no cues.

## What plays

The core reports facts (`GameEvent`). This mod picks a catalog cue:

| Fact | Cue |
|---|---|
| Block broken, or a tool reaction | `break_<class>`, else `break_default` |
| Block placed | `place_<class>`, else `place_default` |
| Footstep (local or a peer) | `step_<class>`, else `step_default` |
| Another player swings | `swing` |
| Menu move or confirm | `menu_click` |
| `/voicetest` | `voicetest` |

`<class>` is the block's sound class (`stone`, `soil`, `wood`, `glass`, `foliage`, `open`). A
local swing is silent: the break, place or tool event carries that sound. Entering or leaving a
world plays nothing. Every cue is played at gain 1; the catalog's own layer gain is unchanged.

Volume, mute and the voice rows stay core settings. This mod adds no options.

## Controls and persisted state

None of its own. Menu clicks come from the core. Nothing is persisted.

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
