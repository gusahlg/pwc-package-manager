# Chat (`pwc.chat`)

The text chat, and the scrollback where the game's messages appear. The base game has no chat of
its own: it carries chat between players (the network message, the server's relay, its limits),
and this package is how you read and write it.

Mod id: `chat`. Part of the
`pwc.chat-commands` bundle, which `pwc.essentials` includes.

## Controls

| Input | Action |
|---|---|
| `§` (the key left of `1`; `` ` `` on a US layout) | Open the chat line |
| `Enter` | Send the line and close |
| `Esc`, or `§` on an empty line | Close without sending |
| `Up`, `Down` | Recall the lines you sent |
| `Tab` | Complete, when a mod offers completions (`pwc.commands` completes command names) |
| `Left`, `Right`, `Home`, `End`, `Backspace`, `Delete`, `Ctrl+Backspace`, `Ctrl+U` | Edit the line |

The chat key is the physical key, whatever your layout prints on it, and it works under every
graphics preset. While the chat is open the world takes no input, and `Esc` closes the chat
instead of leaving the world. A line can hold up to 128 characters.

## What a sent line does

1. Mods that registered a line handler see it first, in the order they registered; the first that
   takes it answers in the scrollback (`pwc.commands` takes `/` commands this way).
2. Otherwise it is chat. On a server it goes to the players near you, or to everyone when it
   starts with `!`. In single player it is echoed (`> hello`).

## The scrollback

Chat (a gold `[global]` tag on global chat, a blue `<name>`, the message), players joining and
leaving, and the game's notices: a restored or damaged save, a failed autosave, audio and
microphone faults, a lost or interrupted connection, a server's mod list. The newest six lines sit
above the chat line in the Full HUD mode (F1), and in every mode while the chat is open. A new world
starts with an empty scrollback.

## For mod authors

`register` provides a `pwc_chat::ChatHandle`. A package that depends on `pwc.chat` gets it with
`registrar.get::<pwc_chat::ChatHandle>()` and can:

- `add_handler(|line, game| …)`: see every sent line with the game state; return
  `Handled::Consumed(lines)` to take it or `Handled::Pass`;
- `set_completer(|line| …)`: answer Tab with a `Completion`;
- `open_with(prefix)`: open the chat on its next frame with `prefix` typed;
- `print(line)`: add a line to the scrollback.

## Settings and persisted state

None. The scrollback and the sent-line history last for one world.

## Dependencies

`pwc.ui-kit ^1.0` (the input line and the scrollback widgets) and the PWC mod API (`pwc-api ^3.0`:
the frame hook, text capture and the message stream).

## Performance

The chat's HUD elements are rebuilt only when the scrollback or the chat line changes; a frame
with nothing new reuses them and allocates nothing.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
