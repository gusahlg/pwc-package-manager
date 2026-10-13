//! A box of cells and its face topology. Cells are indexed x fastest, then y, then z. Outside the
//! box is void: a face that leaves the box has no neighbour.

/// The six face directions in canonical order: −x, −y, −z, +x, +y, +z. Direction `d` and
/// `(d + 3) % 6` are opposite.
pub const DIRS: [(i32, i32, i32); 6] = [(-1, 0, 0), (0, -1, 0), (0, 0, -1), (1, 0, 0), (0, 1, 0), (0, 0, 1)];

/// The opposite of direction `d`.
#[inline]
pub const fn opposite(d: usize) -> usize {
    if d < 3 { d + 3 } else { d - 3 }
}

/// A box of `nx × ny × nz` cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grid {
    /// Cells along x.
    pub nx: u32,
    /// Cells along y.
    pub ny: u32,
    /// Cells along z.
    pub nz: u32,
}

impl Grid {
    /// A box of the given size. Every side is at least one cell.
    pub const fn new(nx: u32, ny: u32, nz: u32) -> Self {
        assert!(nx > 0 && ny > 0 && nz > 0);
        Self { nx, ny, nz }
    }

    /// Cells in the box.
    #[inline]
    pub const fn len(self) -> usize {
        self.nx as usize * self.ny as usize * self.nz as usize
    }

    /// Never true: a box holds at least one cell.
    #[inline]
    pub const fn is_empty(self) -> bool {
        false
    }

    /// Whether a position lies in the box.
    #[inline]
    pub const fn contains(self, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && (x as u32) < self.nx && (y as u32) < self.ny && (z as u32) < self.nz
    }

    /// The index of a position in the box.
    #[inline]
    pub const fn idx(self, x: u32, y: u32, z: u32) -> usize {
        (z as usize * self.ny as usize + y as usize) * self.nx as usize + x as usize
    }

    /// The index of a position, or `None` outside the box.
    #[inline]
    pub const fn at(self, x: i32, y: i32, z: i32) -> Option<usize> {
        if self.contains(x, y, z) { Some(self.idx(x as u32, y as u32, z as u32)) } else { None }
    }

    /// The position of an index.
    #[inline]
    pub const fn pos(self, i: usize) -> (u32, u32, u32) {
        let nx = self.nx as usize;
        let ny = self.ny as usize;
        ((i % nx) as u32, ((i / nx) % ny) as u32, (i / (nx * ny)) as u32)
    }

    /// The neighbour of cell `i` in direction `d`, or `None` outside the box.
    #[inline]
    pub fn step(self, i: usize, d: usize) -> Option<usize> {
        let (x, y, z) = self.pos(i);
        let (dx, dy, dz) = DIRS[d];
        self.at(x as i32 + dx, y as i32 + dy, z as i32 + dz)
    }

    /// The index offset of direction `d` (valid only when the step stays in the box).
    #[inline]
    pub const fn stride(self, d: usize) -> isize {
        let sx = 1isize;
        let sy = self.nx as isize;
        let sz = self.nx as isize * self.ny as isize;
        match d {
            0 => -sx,
            1 => -sy,
            2 => -sz,
            3 => sx,
            4 => sy,
            _ => sz,
        }
    }

    /// The six neighbours of cell `i` in canonical order.
    #[inline]
    pub fn neighbours(self, i: usize) -> [Option<usize>; 6] {
        let (x, y, z) = self.pos(i);
        let (x, y, z) = (x as i32, y as i32, z as i32);
        let mut out = [None; 6];
        for (d, (dx, dy, dz)) in DIRS.iter().enumerate() {
            out[d] = self.at(x + dx, y + dy, z + dz);
        }
        out
    }
}
