//! ASCII views of a candidate, for experiments and reports: one z-slice, liveliness as a ramp.
//! Void at rest is a space, matter at rest a middle dot, and activity climbs the ramp.

use crate::candidate::Candidate;
use crate::matter::VOID;

const RAMP: &[u8] = b".:-=+*#%@";

/// The z-slice `z` of a candidate, top row = highest y.
pub fn slice(c: &dyn Candidate, z: u32) -> String {
    let m = c.matter();
    let g = m.grid;
    let mut out = String::with_capacity((g.nx as usize + 1) * g.ny as usize);
    for y in (0..g.ny).rev() {
        for x in 0..g.nx {
            let i = g.idx(x, y, z);
            let v = c.show(i);
            let ch = if v == 0 {
                if m.get(i) == VOID { ' ' } else { '\u{b7}' }
            } else {
                RAMP[((v as usize - 1) * RAMP.len()) / 255] as char
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

/// One line per tick of a 1-D row (`y`, `z` fixed) over `ticks` ticks: a space-time diagram.
pub fn spacetime(c: &mut dyn Candidate, y: u32, z: u32, ticks: usize) -> String {
    let g = c.matter().grid;
    let mut out = String::new();
    for _ in 0..ticks {
        for x in 0..g.nx {
            let i = g.idx(x, y, z);
            let v = c.show(i);
            out.push(if v == 0 {
                if c.matter().get(i) == VOID { ' ' } else { '\u{b7}' }
            } else {
                RAMP[((v as usize - 1) * RAMP.len()) / 255] as char
            });
        }
        out.push('\n');
        c.step();
    }
    out
}
