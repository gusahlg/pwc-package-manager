//! Copying a box of the world into a law's matter, and mapping between the box and the world.
//! The box is axis-aligned and centred on the cell under the player's feet. Its slice plane is
//! perpendicular to the world axis nearest the player's up, so on any face of a cube world the
//! lab shows the ground the player stands on.

use std::collections::HashMap;

use pwc_law_core::{Grid, Mat, Matter, VOID};
use pwc_mod_api::block::BlockId;
use pwc_mod_api::engine::DVec3;
use pwc_mod_api::player::Player;
use pwc_mod_api::world::World;

/// Where a captured box sits in the world.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Window {
    /// World position of box cell (0, 0, 0).
    pub origin: (i32, i32, i32),
    /// Cells per side.
    pub side: u32,
    /// The world axis nearest the player's up at capture (0 = x, 1 = y, 2 = z).
    pub up: usize,
    /// Whether up points along +axis.
    pub up_positive: bool,
}

impl Window {
    /// The box index of a world position, or `None` outside the box.
    pub fn index(&self, p: (i32, i32, i32)) -> Option<usize> {
        let g = Grid::new(self.side, self.side, self.side);
        g.at(p.0 - self.origin.0, p.1 - self.origin.1, p.2 - self.origin.2)
    }

    /// The world position of a box index.
    pub fn world(&self, i: usize) -> (i32, i32, i32) {
        let (x, y, z) = Grid::new(self.side, self.side, self.side).pos(i);
        (self.origin.0 + x as i32, self.origin.1 + y as i32, self.origin.2 + z as i32)
    }

    /// The index of the box's centre cell.
    pub fn centre(&self) -> usize {
        let h = self.side / 2;
        Grid::new(self.side, self.side, self.side).idx(h, h, h)
    }
}

fn cell_of(p: DVec3) -> (i32, i32, i32) {
    (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
}

/// The dominant axis of a direction and whether it points along +axis.
fn dominant(v: DVec3) -> (usize, bool) {
    let a = [v.x, v.y, v.z];
    let axis = (0..3).max_by(|&i, &j| a[i].abs().total_cmp(&a[j].abs())).unwrap_or(1);
    (axis, a[axis] >= 0.0)
}

/// Copy the `side`³ box centred on the player's feet into matter. Air and unloaded space read as
/// void; every other cell interns the world's own configuration.
pub fn capture(world: &World, player: &Player, side: u32) -> (Matter, Window) {
    let feet = cell_of(player.feet());
    let h = (side / 2) as i32;
    let origin = (feet.0 - h, feet.1 - h, feet.2 - h);
    let (up, up_positive) = dominant(player.up());
    let window = Window { origin, side, up, up_positive };
    let grid = Grid::new(side, side, side);
    let mut matter = Matter::new(grid);
    let registry = world.registry();
    let mut seen: HashMap<BlockId, Mat> = HashMap::new();
    for i in 0..grid.len() {
        let (x, y, z) = window.world(i);
        let id = world.block_at(x, y, z);
        let m = *seen.entry(id).or_insert_with(|| {
            let block = registry.block(id);
            if block.is_empty() { VOID } else { matter.intern(block) }
        });
        matter.set(i, m);
    }
    (matter, window)
}

/// The first solid cell along the player's view within 12 blocks (a voxel walk).
pub fn looked_at(world: &World, player: &Player) -> Option<(i32, i32, i32)> {
    const REACH: f64 = 12.0;
    let o = player.position;
    let d = player.forward();
    let mut c = cell_of(o);
    let dir = [d.x, d.y, d.z];
    let org = [o.x, o.y, o.z];
    let mut cell = [c.0, c.1, c.2];
    let mut step = [0i32; 3];
    let mut next = [f64::INFINITY; 3];
    let mut delta = [f64::INFINITY; 3];
    for a in 0..3 {
        if dir[a] > 0.0 {
            step[a] = 1;
            delta[a] = 1.0 / dir[a];
            next[a] = (cell[a] as f64 + 1.0 - org[a]) / dir[a];
        } else if dir[a] < 0.0 {
            step[a] = -1;
            delta[a] = -1.0 / dir[a];
            next[a] = (org[a] - cell[a] as f64) / -dir[a];
        }
    }
    loop {
        let a = (0..3).min_by(|&i, &j| next[i].total_cmp(&next[j])).unwrap_or(0);
        if next[a] > REACH {
            return None;
        }
        cell[a] += step[a];
        next[a] += delta[a];
        c = (cell[0], cell[1], cell[2]);
        if world.is_solid(c.0, c.1, c.2) {
            return Some(c);
        }
    }
}
