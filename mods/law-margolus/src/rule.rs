//! The block rule, derived rather than chosen. A 2×2×2 block holds eight corners, `c = dx | dy<<1 |
//! dz<<2`, and a particle at corner `c` moves along the body diagonal toward corner `c ^ 7`: its
//! velocity is +1 on every axis whose bit is 0 and −1 on every axis whose bit is 1. A block's state
//! is the set of occupied corners, one byte.
//!
//! - **Free flight** `F` sends every particle to the opposite corner (`c ^ 7`).
//! - **Collision** `G` permutes velocity sets before the flight. It is not a table someone wrote: it
//!   is the unique bijection that conserves particle number and momentum, commutes with all 48
//!   symmetries of the cube, and leaves no state fixed that some such bijection could move. Exactly
//!   one exists for every momentum class that admits one (the experiments check it), and it is its own
//!   inverse. Head-on pairs are *not* in it: no deflection of a head-on pair is isotropic.
//! - **Walls** turn the block into a mirror: a particle crosses the block along every axis under whose
//!   reflection the block's walls are symmetric and turns back along the others. A flat wall is a
//!   specular mirror, an inside edge reflects two axes, a lone wall corner reflects all three.
//!
//! Every map here is an involution, so a whole tick is undone by applying the same partition again.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// How walls reflect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reflection {
    /// Along the axes the block's walls are mirror-symmetric under, particles keep going (specular).
    Mirror,
    /// A block that touches a wall leaves its particles in place: every velocity reverses (the
    /// brief's literal rule, kept for comparison).
    Bounce,
}

/// What particles that meet in empty space do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Collision {
    /// The collision forced by symmetry and momentum ([`collision`]).
    Derived,
    /// The brief's literal billiard-ball rule, kept for comparison: a head-on pair turns 180° about
    /// z (the achiral choice, so the map stays an involution) and every other pattern of two or more
    /// particles stays where it is, which reverses its momentum.
    Literal,
}

/// A whole block rule: how particles collide and how walls reflect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rule {
    /// Collisions in empty space.
    pub collision: Collision,
    /// Reflection at walls.
    pub reflection: Reflection,
}

impl Rule {
    /// The law: the derived collision and mirror walls.
    pub const LAW: Rule = Rule { collision: Collision::Derived, reflection: Reflection::Mirror };
    /// The brief read literally: head-on deflection, everything else and every walled block unchanged.
    pub const LITERAL: Rule = Rule { collision: Collision::Literal, reflection: Reflection::Bounce };
    /// Every combination, for exhaustive checks.
    pub const ALL: [Rule; 4] = [
        Rule::LAW,
        Rule { collision: Collision::Derived, reflection: Reflection::Bounce },
        Rule { collision: Collision::Literal, reflection: Reflection::Mirror },
        Rule::LITERAL,
    ];
}

/// Velocity of corner `c` along axis `a`.
#[inline]
pub const fn vel(c: u8, a: u8) -> i32 {
    1 - 2 * ((c >> a) & 1) as i32
}

/// Every corner of a set moved by xor with `m`.
pub const fn flip(s: u8, m: u8) -> u8 {
    let mut out = 0u8;
    let mut c = 0u8;
    while c < 8 {
        if (s >> c) & 1 == 1 {
            out |= 1 << (c ^ m);
        }
        c += 1;
    }
    out
}

/// Total velocity of a set of corners.
pub fn momentum(s: u8) -> [i32; 3] {
    let mut p = [0; 3];
    for c in 0..8u8 {
        if (s >> c) & 1 == 1 {
            for (a, pa) in p.iter_mut().enumerate() {
                *pa += vel(c, a as u8);
            }
        }
    }
    p
}

/// A symmetry of the cube as a permutation of its corners, and whether it is a rotation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sym {
    /// Image of each corner.
    pub map: [u8; 8],
    /// A proper rotation (determinant +1) rather than a reflection.
    pub proper: bool,
}

impl Sym {
    /// The image of a set of corners.
    pub fn act(&self, s: u8) -> u8 {
        (0..8).filter(|&c| (s >> c) & 1 == 1).fold(0, |o, c| o | 1 << self.map[c as usize])
    }
}

/// The 48 symmetries of the cube: every permutation of the axes with every choice of signs.
pub fn symmetries() -> Vec<Sym> {
    const PERMS: [[u8; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    const ODD: [bool; 6] = [false, true, true, false, false, true];
    let mut out = Vec::with_capacity(48);
    for (p, &odd) in PERMS.iter().zip(&ODD) {
        for signs in 0..8u8 {
            let mut map = [0u8; 8];
            for (c, m) in map.iter_mut().enumerate() {
                for (a, &from) in p.iter().enumerate() {
                    *m |= (((c as u8 >> from) & 1) ^ ((signs >> a) & 1)) << a;
                }
            }
            out.push(Sym { map, proper: odd == (signs.count_ones() % 2 == 1) });
        }
    }
    out
}

/// A momentum class: particle number, total momentum, and its block states in increasing order.
pub type Class = ((u32, [i32; 3]), Vec<u8>);

/// The block states grouped by particle number and momentum, in a fixed order.
pub fn classes() -> Vec<Class> {
    let mut by: BTreeMap<(u32, [i32; 3]), Vec<u8>> = BTreeMap::new();
    for s in 0..=255u8 {
        by.entry((s.count_ones(), momentum(s))).or_default().push(s);
    }
    by.into_iter().collect()
}

/// The symmetries that fix a momentum (and so map its class onto itself).
pub fn stabilizer(group: &[Sym], member: u8) -> Vec<Sym> {
    let p = momentum(member);
    group.iter().copied().filter(|g| momentum(g.act(member)) == p).collect()
}

/// Every fixed-point-free permutation of `members` that commutes with every symmetry in `group`
/// (which must map `members` onto itself), as the image of each member in order.
pub fn commuting_derangements(members: &[u8], group: &[Sym]) -> Vec<Vec<u8>> {
    fn pos(members: &[u8], s: u8) -> usize {
        members.iter().position(|&m| m == s).expect("the group keeps the class")
    }
    fn go(members: &[u8], group: &[Sym], f: &[Option<u8>], out: &mut Vec<Vec<u8>>) {
        let Some(i) = f.iter().position(Option::is_none) else {
            out.push(f.iter().map(|t| t.expect("assigned")).collect());
            return;
        };
        for &t in members {
            if t == members[i] || f.contains(&Some(t)) {
                continue;
            }
            // f(g·s) = g·f(s) fixes f on the whole orbit of s.
            let mut g_f = f.to_vec();
            let mut ok = true;
            for g in group {
                let (gs, gt) = (pos(members, g.act(members[i])), g.act(t));
                match g_f[gs] {
                    Some(v) if v != gt => ok = false,
                    Some(_) => {}
                    None if gt == members[gs] || g_f.contains(&Some(gt)) => ok = false,
                    None => g_f[gs] = Some(gt),
                }
                if !ok {
                    break;
                }
            }
            if ok {
                go(members, group, &g_f, out);
            }
        }
    }
    let mut out = Vec::new();
    go(members, group, &vec![None; members.len()], &mut out);
    out
}

/// The collision `G`: for every class with exactly one isotropic derangement, that derangement;
/// every other state is left alone.
pub fn collision() -> [u8; 256] {
    let group = symmetries();
    let mut g = [0u8; 256];
    for (s, v) in g.iter_mut().enumerate() {
        *v = s as u8;
    }
    for (_, members) in classes() {
        if members.len() < 2 {
            continue;
        }
        let ds = commuting_derangements(&members, &stabilizer(&group, members[0]));
        if let [d] = ds.as_slice() {
            for (&s, &t) in members.iter().zip(d) {
                g[s as usize] = t;
            }
        }
    }
    g
}

/// The axes a particle keeps crossing in a block whose wall corners are `w`: every axis under whose
/// reflection the walls are symmetric.
pub const fn mirror_axes(w: u8) -> u8 {
    let mut m = 0u8;
    let mut a = 0u8;
    while a < 3 {
        if flip(w, 1 << a) == w {
            m |= 1 << a;
        }
        a += 1;
    }
    m
}

/// The whole block rule as one table, indexed `wall_corners << 8 | particles`: free flight after
/// the collision in empty space, the mirror in a walled block. States with a particle inside a wall
/// map to themselves (they never occur).
pub fn table(rule: Rule) -> &'static [u8; 65536] {
    static TABLES: [OnceLock<Box<[u8; 65536]>>; 4] = [const { OnceLock::new() }; 4];
    let k = Rule::ALL.iter().position(|&r| r == rule).expect("a listed rule");
    TABLES[k].get_or_init(|| {
        let g = collision();
        let mut t = Box::new([0u8; 65536]);
        for w in 0..256usize {
            let m = mirror_axes(w as u8);
            for s in 0..256usize {
                let b = s as u8;
                t[w << 8 | s] = if s & w != 0 {
                    b
                } else if w == 0 {
                    match rule.collision {
                        Collision::Derived => flip(g[s], 7),
                        Collision::Literal if b.count_ones() < 2 => flip(b, 7),
                        Collision::Literal if b.count_ones() == 2 && flip(b, 7) == b => flip(b, 3),
                        Collision::Literal => b,
                    }
                } else {
                    match rule.reflection {
                        Reflection::Mirror => flip(b, m),
                        Reflection::Bounce => b,
                    }
                };
            }
        }
        t
    })
}
