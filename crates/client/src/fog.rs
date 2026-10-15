//! The darkening of what the local side does not see in the view (`Game_Dll.dll` `0x10071a10`, then
//! `cad800.dll` `0x10002ba0`). Each frame a light map is built on a grid of corners 16 pixels apart
//! across and 8 down the screen: 0x80 where the cell is seen, 0 where it is not, 0x40 on a seen
//! cell next to an unseen one, the corners in between averaged. The driver's per-pixel interpolation
//! and RGB565 row carry are confirmed against `cad800.dll` `0x10002ba0` (see [`factor`]).

/// Light of a seen cell, and of one on the edge of what is seen.
pub const LIT: u8 = 0x80;
pub const EDGE: u8 = 0x40;

/// The light map: `stride` corners per row; corner `(row, col)` sits at screen pixel
/// `((col - ORIGIN) * 16, (row - ORIGIN) * 8)` (the driver reads it from 8 rows and 8 columns in).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Light {
    pub stride: usize,
    pub corners: Vec<u8>,
}

pub const ORIGIN: i32 = 8;

impl Light {
    pub fn at(&self, row: i32, col: i32) -> u8 {
        self.corners[row as usize * self.stride + col as usize]
    }
}

/// The light map for a view whose top left is `view` in the game's screen coordinates (map pixel
/// `(x, y)` at `x - y`, `(x + y) / 2`) and `size` pixels wide and high, over a map of `map_size`
/// cells, `seen(x, y)` telling whether the local side sees cell `(x, y)` (`0x10071a10`; the game's
/// grid is 80 corners wide for 800 pixels, this one as wide as the view needs).
pub fn light(
    view: (i32, i32),
    size: (i32, i32),
    map_size: (i32, i32),
    seen: impl Fn(i32, i32) -> bool,
) -> Light {
    let (vx, vy) = view;
    let cols = size.0 >> 4;
    let rows = size.1 >> 3;
    let stride = (cols + 12) as usize;
    let height = (rows + 14) as usize;
    let mut l = vec![LIT; stride * height];
    let s = stride as i32;
    let at = |r: i32, c: i32| (r * s + c) as usize;
    let visible =
        |x: i32, y: i32| x >= 4 && y >= 4 && x < map_size.0 - 4 && y < map_size.1 - 4 && seen(x, y);

    // the corners on the cells' vertices
    let u1 = vy >> 3;
    let u12 = (u1 - 3) & 1;
    let i6 = vx >> 4;
    let mut u5 = (((((u1 - 3) >> 1) ^ u12) * -2 + 1) - i6) & 3;
    let i2 = ((u5 - 3) * 16 + vx) >> 1;
    let i9 = u12 * 8 - 0x18;
    let mut cy0 = (i9 - i2 + vy) >> 5;
    let mut col0 = u5 + 5;
    let mut cx0 = (i9 + i2 + vy) >> 5;
    let mut row = u12 + 5;
    let mut phase = u5;
    while row <= rows + 11 {
        let (mut col, mut cx, mut cy) = (col0, cx0, cy0);
        while col <= cols + 11 {
            let i = at(row, col);
            if !visible(cx, cy) {
                l[i] = 0;
            } else if l[i] > EDGE
                && !(seen(cx, cy + 1) && seen(cx + 1, cy) && seen(cx, cy - 1) && seen(cx - 1, cy))
            {
                l[i] = EDGE;
            }
            col += 4;
            cx += 1;
            cy -= 1;
        }
        if phase < 2 {
            col0 += 2;
            cx0 += 1;
        } else {
            col0 -= 2;
            cy0 += 1;
        }
        phase ^= 2;
        row += 2;
    }

    // the corners between four of them
    let mut step = if u5 < 2 {
        u5 += 4;
        -2
    } else {
        2
    };
    let mut c0 = u5 + 5;
    let mut r = u12 + 7;
    while r < rows + 10 {
        let mut c = c0;
        while c < cols + 10 {
            let sum = l[at(r - 2, c)] as u32
                + l[at(r + 2, c)] as u32
                + l[at(r, c + 2)] as u32
                + l[at(r, c - 2)] as u32;
            l[at(r, c)] = (sum >> 2) as u8;
            c += 4;
        }
        c0 += step;
        step = -step;
        r += 2;
    }

    // the corners between two across, then between two down
    let last_col = cols + 8;
    let last_row = rows + 8;
    let mut r = (u1 & 1) + 8;
    let first_col = ((i6 - 1) & 1) + 8;
    while r <= last_row {
        let mut c = first_col;
        while c <= last_col {
            l[at(r, c)] = ((l[at(r, c - 1)] as u32 + l[at(r, c + 1)] as u32) >> 1) as u8;
            c += 2;
        }
        r += 2;
    }
    let mut r = ((u1 - 1) & 1) + 8;
    while r <= last_row {
        for c in 8..=last_col {
            l[at(r, c)] = ((l[at(r - 1, c)] as u32 + l[at(r + 1, c)] as u32) >> 1) as u8;
        }
        r += 2;
    }
    Light { stride, corners: l }
}

/// Per-pixel multiplier in 32ths, reconstructed from `cad800.dll` `0x10002ba0`: a 16 x 8 tile
/// takes its four corners; all-zero corners are halved, all [`LIT`] corners are unchanged, and
/// mixed values are interpolated in 8.8 fixed point. Channel scaling and its row carry are applied
/// by the renderer's shader.
pub fn factor(light: &Light, x: i32, y: i32) -> i32 {
    let (tx, ty) = (x >> 4, y >> 3);
    let (row, col) = (ty + ORIGIN, tx + ORIGIN);
    let (tl, tr) = (light.at(row, col) as i32, light.at(row, col + 1) as i32);
    let (bl, br) = (
        light.at(row + 1, col) as i32,
        light.at(row + 1, col + 1) as i32,
    );
    if tl == 0 && tr == 0 && bl == 0 && br == 0 {
        return 16;
    }
    if [tl, tr, bl, br].iter().all(|&v| v == LIT as i32) {
        return 32;
    }
    let (px, py) = (x & 15, y & 7);
    let start = (tl + 0x7f) << 5;
    let across = (tr - tl) * 2;
    let down = (bl - tl) * 4;
    let bend = ((br - tr) - (bl - tl)) >> 2;
    (start + py * down + px * (across + py * bend)) >> 8
}
