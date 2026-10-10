# Game UI (`pwc.game-ui`)

The in-world HUD of the essentials: the **information HUD** the base game used to draw (the
reticle, the coordinates, the frame rate, the player count and the loading and connection lines),
and the **facing indicator**: which way you look along the world's own X, Y and Z axes, the axes
`/tp` coordinates and the coordinate readout use. The base game draws no HUD text of its own.

Mod id: `game_ui`. Part of `pwc.essentials`.

## The information HUD

The same layout and text the game had, by HUD mode (F1 cycles Full, Minimal and Off):

- **The reticle** in the middle of the screen: Full and Minimal.
- **"Loading world…"** while a server sends the world you joined, then **"Loading terrain…"** until
  the ground around you has loaded, at the top: Full and Minimal. They replace the readouts below.
- Full only: **the coordinates** at the top centre (with `CRUISE … km/s` while cruising), shrunk to
  fit between the frame rate and the minimap; **the frame rate** at the top left; on a server,
  **the player count** (and ping) under the minimap.
- **"connection interrupted"** under the top line while the server has gone quiet: Full and Minimal.

## The facing indicator

In the top-right corner, under the minimap:

- **A gizmo**: three short axes from a common centre show where +X (red), +Y (green) and +Z (blue)
  point on your screen. An axis pointing into the screen is drawn dimmer, and nearer axes are
  drawn over farther ones. Looking along +X with +Y up, +Z points right and +X is a dot in the
  middle.
- **The facing in words**: the axis you look along most, with its sign (`facing +X`), and a second
  one once the view leans toward it by more than 22.5° (`facing -Z +Y`: mostly along -Z, tilted
  up).
- **The view direction** as a unit vector, to two decimals: `(+0.00, +0.50, -0.87)`.

It follows your view (yaw and pitch, and the body's up on a round world), not a detached free
camera. It is drawn in the Full and Minimal HUD modes.

## Controls and settings

None. A build without the package has no reticle and no readouts.

## Persisted state

Nothing.

## Dependencies

None besides the PWC mod API (`pwc-api ^3.0`: the HUD facts).

## Performance

The HUD elements are rebuilt only when what they show changes (a shown digit of the coordinates,
frame rate or player count; the direction to the hundredth, the gizmo to the pixel, or the screen
size). A frame with nothing new reuses them and allocates nothing.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
