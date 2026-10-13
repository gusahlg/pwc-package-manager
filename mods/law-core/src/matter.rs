//! Real matter in a box: each cell holds a palette index, and the palette holds the game's own
//! configurations (`material::Block`) with their readings. Every constant a candidate law uses is
//! one of these readings, all taken with the one function the universe already has, `fit_raw`:
//!
//! - a configuration's **cohesion** (mean internal pair fit) and its responses to the law's probes;
//! - a **bond**: the mean fit across a face between two configurations. Positive means the two
//!   attract (a spring, a coupling, a conductor), negative that they repel (an insulator, an
//!   inhibition). Void bonds to nothing.
//!
//! All readings are in 1/256 of `QUANTUM`, the scale of `material::probe_response`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::grid::Grid;
use crate::material::{cohesion, fit_raw, probe_response, Block, Configuration, Law, QUANTUM};

/// Materials whose bonds live in the dense table (4 MiB); later ones use the sparse cache.
const DENSE: usize = 1024;

/// A dense slot not yet computed (no bond reaches it: readings lie within −2048..=1152).
const UNKNOWN: i32 = i32::MIN;

/// A palette index. [`VOID`] is empty space.
pub type Mat = u16;

/// The empty configuration.
pub const VOID: Mat = 0;

/// What a configuration reads as, cached once per palette entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Reading {
    /// Occurrences held.
    pub len: u8,
    /// Mean internal pair fit (Q8 of `QUANTUM`); 0 for fewer than two occurrences.
    pub cohesion: i32,
    /// Summed internal fit in raw `fit_raw` units (`Block::internal_fit`).
    pub internal: i64,
    /// Response to the law's light probe.
    pub light: i32,
    /// Response to the law's glow probe.
    pub glow: i32,
    /// Response to the law's grip probe.
    pub grip: i32,
}

/// A box of matter: cells, the palette and the cached readings and bonds.
#[derive(Clone)]
pub struct Matter {
    /// The box.
    pub grid: Grid,
    cells: Vec<Mat>,
    palette: Vec<Block>,
    readings: Vec<Reading>,
    /// Bonds among the first `stride` materials, filled on first use ([`UNKNOWN`] until then).
    /// The stride doubles with the palette up to [`DENSE`].
    dense: Vec<Cell<i32>>,
    stride: usize,
    /// Bonds involving later materials, filled on first use. A hot world can invent thousands
    /// of configurations; most pairs among them never touch.
    sparse: RefCell<HashMap<(Mat, Mat), i32>>,
    index: HashMap<Configuration, Mat>,
}

impl Matter {
    /// A box of void.
    pub fn new(grid: Grid) -> Self {
        let mut index = HashMap::new();
        index.insert(Block::default().configuration(), VOID);
        Self {
            grid,
            cells: vec![VOID; grid.len()],
            palette: vec![Block::default()],
            readings: vec![Reading::default()],
            dense: vec![Cell::new(UNKNOWN); 16 * 16],
            stride: 16,
            sparse: RefCell::new(HashMap::new()),
            index,
        }
    }

    /// The palette index of a configuration, adding it (and its readings and bonds) when new.
    pub fn intern(&mut self, block: &Block) -> Mat {
        let key = block.configuration();
        if let Some(&m) = self.index.get(&key) {
            return m;
        }
        let m = Mat::try_from(self.palette.len()).expect("palette full");
        let law = Law::current();
        let reading = Reading {
            len: block.len() as u8,
            cohesion: cohesion(block),
            internal: block.internal_fit(),
            light: probe_response(block, law.probes.light),
            glow: probe_response(block, law.probes.glow),
            grip: probe_response(block, law.probes.grip),
        };
        self.palette.push(block.clone());
        self.readings.push(reading);
        self.index.insert(key, m);
        let n = self.palette.len();
        if n > self.stride && self.stride < DENSE {
            let stride = (self.stride * 2).min(DENSE);
            let dense = vec![Cell::new(UNKNOWN); stride * stride];
            for a in 0..self.stride {
                for b in 0..self.stride {
                    dense[a * stride + b].set(self.dense[a * self.stride + b].get());
                }
            }
            self.dense = dense;
            self.stride = stride;
        }
        m
    }

    /// The material of cell `i`.
    #[inline]
    pub fn get(&self, i: usize) -> Mat {
        self.cells[i]
    }

    /// Set cell `i` to material `m` (an index this box interned).
    #[inline]
    pub fn set(&mut self, i: usize, m: Mat) {
        debug_assert!((m as usize) < self.palette.len());
        self.cells[i] = m;
    }

    /// Set the cell at a position; positions outside the box are ignored.
    pub fn put(&mut self, x: i32, y: i32, z: i32, m: Mat) {
        if let Some(i) = self.grid.at(x, y, z) {
            self.cells[i] = m;
        }
    }

    /// Fill an inclusive box of positions with `m` (clipped to the grid).
    pub fn fill(&mut self, lo: (i32, i32, i32), hi: (i32, i32, i32), m: Mat) {
        for z in lo.2..=hi.2 {
            for y in lo.1..=hi.1 {
                for x in lo.0..=hi.0 {
                    self.put(x, y, z, m);
                }
            }
        }
    }

    /// Every cell, in index order.
    #[inline]
    pub fn cells(&self) -> &[Mat] {
        &self.cells
    }

    /// The configuration of a material.
    #[inline]
    pub fn block(&self, m: Mat) -> &Block {
        &self.palette[m as usize]
    }

    /// The readings of a material.
    #[inline]
    pub fn reading(&self, m: Mat) -> &Reading {
        &self.readings[m as usize]
    }

    /// The bond across a face between two materials (0 when either is void).
    #[inline]
    pub fn bond(&self, a: Mat, b: Mat) -> i32 {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        if (b as usize) < self.stride {
            let slot = &self.dense[a as usize * self.stride + b as usize];
            let v = slot.get();
            if v != UNKNOWN {
                return v;
            }
            let v = bond_of(&self.palette[a as usize], &self.palette[b as usize]);
            slot.set(v);
            return v;
        }
        *self
            .sparse
            .borrow_mut()
            .entry((a, b))
            .or_insert_with(|| bond_of(&self.palette[a as usize], &self.palette[b as usize]))
    }

    /// Materials in the palette, void included.
    #[inline]
    pub fn palette_len(&self) -> usize {
        self.palette.len()
    }

    /// Elements held in the box: a conserved quantity of any composition-preserving law.
    pub fn occurrences(&self) -> i64 {
        self.cells.iter().map(|&m| i64::from(self.readings[m as usize].len)).sum()
    }
}

/// Mean fit across a face between two configurations, Q8 of `QUANTUM`; 0 when either is empty.
pub fn bond_of(a: &Block, b: &Block) -> i32 {
    let (na, nb) = (a.len() as i64, b.len() as i64);
    if na == 0 || nb == 0 {
        return 0;
    }
    let mut sum = 0i64;
    for &x in a.elements() {
        for &y in b.elements() {
            sum += i64::from(fit_raw(x, y));
        }
    }
    (sum * 256 / (na * nb * i64::from(QUANTUM))) as i32
}
