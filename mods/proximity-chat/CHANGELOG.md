# Changelog — pwc.proximity-chat

## 1.1.0 (unreleased)

- Voice Chat is this package's option (the Audio page; `pwc.proximity-chat.voice_enabled`): the
  core no longer has a voice_enabled setting, and its old line is read once. No description or group
  of its own.

## 1.0.0

- Mod id `proximity_chat`, in the Essentials group, enabled by default. Requires `pwc-api ^2.1`.
- Hold `V` (`voice.talk`) to send on the `"voice"` channel. Audible peers are the visible interest
  set. The microphone stays closed when voice is disabled or there is no connection.
- Licensed `Apache-2.0 OR MIT` at your option; both texts are in `LICENSES/`.
