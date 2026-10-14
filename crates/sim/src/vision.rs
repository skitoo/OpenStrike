//! Sight (`Game_Dll.dll` `0x1004dd40` and around): one byte per cell (`0x101c54e6`, 256 x 256),
//! a bit per side in each nibble. A unit's sight sets `0x11 << side` on the cells its rays reach:
//! the low nibble is what each side sees, the high one what it has seen since the last refresh.
//! Every 64 ticks (`0x1004e220`) the high nibble becomes the low one and is cleared, so a cell stays
//! seen until a refresh finds no unit looked at it in between.

use crate::grid::{CELLS, Grid, SIDE};

/// Cells a ray stops after (`0x1800`), and the one that lets it go on over low ground (`0x2000`:
/// from such a cell on, only `0x800` stops it).
const STOPS: u32 = 0x1800;
const LOW: u32 = 0x2000;
const HIGH: u32 = 0x800;

/// The ray ends for each sight radius (`circ64.dat`, see `openstrike_formats::circles`): offsets in
/// cells; index 0 is empty.
pub type Circles = Vec<Vec<(i8, i8)>>;

/// One ray from cell `(x, y)` towards `(x + dx, y + dy)` (`0x1004dd40`), setting `bits` on every
/// cell it reaches. Cells are stepped through as the game does, by linear index in the 256-wide
/// array with a 16.16 accumulator. A ray that would leave the array stops (the game does not check).
pub fn cast(seen: &mut [u8], grid: &Grid, x: i32, y: i32, dx: i32, dy: i32, bits: u8) {
    let (sx, mut n) = if dx < 1 { (-1, -dx) } else { (1, dx) };
    let (sy, ady) = if dy < 1 { (-(SIDE), -dy) } else { (SIDE, dy) };
    let (major, minor, frac) = if ady < n {
        (sx, sy, ady * 0xffff / n)
    } else if n < ady {
        let frac = n * 0xffff / ady;
        n = ady;
        (sy, sx, frac)
    } else {
        (sx + sy, sy, 0)
    };
    let mut at = y * SIDE + x;
    let Some(start) = cell(at) else { return };
    seen[start] |= bits;
    if n == 0 {
        return;
    }
    // from a low cell on, only high ground stops the ray
    let mut low = grid.at_linear(at) & LOW != 0;
    let mut acc = frac + 0x8000;
    at += major;
    if acc > 0x10000 {
        acc = frac - 0x8000;
        at += minor;
    }
    loop {
        let Some(i) = cell(at) else { return };
        seen[i] |= bits;
        let flags = grid.at_linear(at);
        if !low && flags & LOW != 0 {
            low = true;
        }
        if low {
            if flags & HIGH != 0 {
                return;
            }
        } else if flags & STOPS != 0 {
            return;
        }
        at += major;
        acc += frac;
        if acc > 0x10000 {
            at += minor;
            acc -= 0x10000;
        }
        n -= 1;
        if n == 0 {
            return;
        }
    }
}

fn cell(at: i32) -> Option<usize> {
    usize::try_from(at).ok().filter(|&i| i < CELLS)
}

/// A unit's all-round sight (`0x1004e070`): a ray to each end of its radius' circle, for `side`
/// (0-3; none beyond).
pub fn circle(seen: &mut [u8], grid: &Grid, circles: &Circles, x: i32, y: i32, radius: i32, side: u8) {
    if side > 3 {
        return;
    }
    let bits = 0x11u8 << side;
    for &(dx, dy) in circles.get(radius as usize).map_or(&[][..], |c| &c[..]) {
        cast(seen, grid, x, y, dx as i32, dy as i32, bits);
    }
}

/// The refresh every 64 ticks (`0x1004e220`, when `(tick + 4) & 63 == 0`): over the map's cells,
/// what was seen since the last one becomes what is seen.
pub fn refresh(seen: &mut [u8], width: i32, height: i32) {
    for y in 0..height {
        for x in 0..width {
            let v = &mut seen[(y * SIDE + x) as usize];
            *v = (*v & 0xf0) >> 4;
        }
    }
}
