# Changelog — pwc.law-polyakov

## 0.1.0

- First release. Fields on faces and edges: an `i16` electric flux on each face, an `i16` magnetic
  flux on each edge and an `i16` charge per cell, all wrapping (compact `Z_65536` gauge group).
  One tick is Ohm (the only lossy operator) and then the integer Yee shears, so Gauss's law and
  `div B = 0` hold exactly and the field part runs backwards (`step_back`, and `unstep` where no
  face conducts). `ε⁻¹` comes from the light probe, carriers from the glow probe and the
  breakdown flux of a junction from the negative bond. The poke is a charge pair joined by its
  flux string. Experiments and a bench in the physics lab. A library: it registers nothing.
  Requires `pwc-api ^3.1` and `pwc.law-core ^0.1`.
