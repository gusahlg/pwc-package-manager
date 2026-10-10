# Game UI (`pwc.game-ui`)

In-world HUD pieces of the essentials. The first one is the **facing indicator**: which way you
look along the world's own X, Y and Z axes, the axes `/tp` coordinates and the coordinate readout
use. Later HUD pieces of the same kind join this package.

Mod id: `game_ui`. Part of `pwc.essentials`.

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
camera. It is drawn whenever mod HUDs are shown (the Full and Minimal HUD modes).

## Controls and settings

None.

## Persisted state

Nothing.

## Dependencies

None besides the PWC mod API (`pwc-api ^2.0`).

## Performance

The HUD elements are rebuilt only when what they show changes (the direction to the hundredth,
the gizmo to the pixel, or the screen size). A frame with a still view reuses them and allocates
nothing.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSES/Apache-2.0.txt](LICENSES/Apache-2.0.txt)), or
- MIT licence ([LICENSES/MIT.txt](LICENSES/MIT.txt))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this package by you, as defined in the Apache-2.0 licence, shall be dual licensed as above, without
any additional terms or conditions.
