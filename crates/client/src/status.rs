//! The status bars the game draws under a selected unit (`Game_Dll.dll` `0x10003580`, the tank's
//! vtable `+0x23c`, called in the pass after the hulls for selected units only): lengths from
//! `0x10002cf0`, colour of the hit points from `0x10009ea0`, bars from `0x10009ff0`, each row a
//! horizontal line (driver `+0xaa28`, `cad800.dll` `0x10001420`). Colours are `user.pl` entries
//! (`0x10387cb0` + 2 * entry).
//!
//! Not drawn yet: the weapon boxes of the local player's units (`0x1000a2b0`), there are no
//! weapons yet.

/// Background of every bar.
pub const BACKGROUND: usize = 1;
/// Hit points of the local player's units: unhurt, damaged, badly damaged.
pub const HEALTHY: usize = 2;
pub const DAMAGED: usize = 3;
pub const WRECKED: usize = 4;
/// Ammunition bars.
pub const AMMO: usize = 5;
/// Hit points of another player's units: not hostile to the local player, hostile.
pub const OTHER: usize = 6;
pub const HOSTILE: usize = 7;
/// The crew bar (which veterans extend), then the crew's own condition over it.
pub const CREW_BAR: usize = 7;
pub const CREW: usize = 8;

/// Lengths at or above this hide a bar; 0xff hides them all.
const HIDDEN: i32 = 0x3f;
const NONE: i32 = 0xff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Local,
    /// A player hostile to the local one (`[0x101ff390 + 2 * local]`, see `mission::hostile`).
    Hostile,
    Other,
}

/// What the bars show of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub owner: Owner,
    pub hp: i32,
    /// `Health` of the type.
    pub max_hp: i32,
    /// Hit points under which the bar shows as damaged: `MoveDamageHP` for vehicles (type flag 8),
    /// else half of `Health`.
    pub damaged_below: i32,
    /// Crew condition, 0..0x8000.
    pub crew: i32,
    /// Veteran crew (`+0x1a & 0xc000`): the crew bar is longer.
    pub veteran: bool,
    pub ammo: [i32; 2],
    /// `Ammo1`, `Ammo2` of the type.
    pub max_ammo: [i32; 2],
}

/// One row of a bar: `len` pixels from `(x, y)` in `user.pl` colour `colour`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub x: i32,
    pub y: i32,
    pub len: i32,
    pub colour: usize,
}

/// Crew condition shown by the crew bar: a veteran's is raised by half, to at least 0xccd first
/// and at most 0x8000 (`0x10002cf0`).
fn shown_crew(crew: i32, veteran: bool) -> i32 {
    if !veteran {
        return crew;
    }
    (crew.max(0xccd) + (crew >> 1)).min(0x8000)
}

/// The rows drawn for a unit whose ground point is at screen `at`, `half` being the type's
/// `Selector`. From the top, `half * 3 / 4` below the point and `2 * half` wide: each ammunition
/// bar (local player only), the crew bar, then the hit points on two rows.
pub fn bars(at: (i32, i32), half: i32, s: &Status) -> Vec<Line> {
    let width = half * 2;
    let hp = s.hp * width / s.max_hp;
    let crew = (width * shown_crew(s.crew, s.veteran)) >> 15;
    let ammo = s.ammo.iter().zip(s.max_ammo).map(|(&a, max)| match (s.owner, max) {
        (Owner::Local, max) if max != 0 => a * width / max,
        _ => NONE,
    });
    let hp_colour = match s.owner {
        Owner::Hostile => HOSTILE,
        Owner::Other => OTHER,
        Owner::Local if s.hp < s.max_hp >> 2 => WRECKED,
        Owner::Local if s.hp < s.damaged_below => DAMAGED,
        Owner::Local => HEALTHY,
    };

    let (x0, mut y) = (at.0 - half, at.1 + ((half * 3) >> 2));
    let mut out = Vec::new();
    if hp == NONE {
        return out;
    }
    let bar = |out: &mut Vec<Line>, y: i32, len: i32, colour: usize| {
        if width - len > 0 {
            out.push(Line { x: x0 + len, y, len: width - len, colour: BACKGROUND });
        }
        if len != 0 {
            out.push(Line { x: x0, y, len, colour });
        }
    };
    for len in ammo {
        if len < HIDDEN {
            bar(&mut out, y, len, AMMO);
            y += 1;
        }
    }
    if crew < HIDDEN {
        bar(&mut out, y, crew, CREW_BAR);
        let own = (width * s.crew) >> 15;
        if own != 0 {
            out.push(Line { x: x0, y, len: own, colour: CREW });
        }
        y += 1;
    }
    for row in [y, y + 1] {
        bar(&mut out, row, hp, hp_colour);
    }
    out
}

/// Where the group number of a unit with bars is drawn (`0x10009ff0`, when the unit's group,
/// copied to `+0xda`, is not 0): the glyph's origin, its anchor still to add, at the bars' right
/// end, 5 pixels above their top.
pub fn group_at(at: (i32, i32), half: i32) -> (i32, i32) {
    (at.0 - 4 + half, at.1 + ((half * 3) >> 2) - 5)
}
