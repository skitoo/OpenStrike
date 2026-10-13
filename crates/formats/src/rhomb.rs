//! Ground rhombs: `mappic.rs2` (in each climate archive) holds raw 8-bit rhombs of 1024 bytes,
//! coloured with the 64 palettes of `rhomb.pl`: two ramps of 32 brightness levels (0 dark, 31
//! bright), palettes 0-31 and 32-63, the second slightly brighter.
//!
//! Loader in the original game: `Game_Dll.dll` `0x1004fac0` (it copies `rhomb.pl` into the drawing
//! table at `cad800.dll` `0x10014e5c`). Drawing (`Game_Dll.dll` `0x100700e0` -> `cad800.dll`
//! `0x10001ea0` -> `0x10003430`), for each cell at least 4 cells away from the map edge:
//!
//! - the rhomb's top vertex is at screen `((cx - cy) * 32, (cx + cy) * 16)` (iterator `0x1004be20`);
//!   its rows are 3, 7, ..., 63 then 61, 57, ..., 1 pixels wide, all centred on the top vertex;
//! - the brightness level is interpolated in 8.8 fixed point between the levels (bits 11-15 of the
//!   map word) of the cell (top vertex), the cell at x + 1 (right vertex), at y + 1 (left vertex) and
//!   at (x + 1, y + 1) (bottom vertex), over the two triangles above and below the horizontal
//!   diagonal;
//! - each pixel uses palette `level` or `level + 32` in a checkerboard. The checkerboard's phase is a
//!   global flag that flips with every row the game draws, so it depends on the viewport and on what
//!   was drawn before; here it alternates from cell to cell along x, as in a full screen row.
//!
//! Cells in the 4-cell border are drawn with an empty rhomb at level 0 (`0x100701a0`), i.e. black.

use crate::iso::Projection;
use crate::map::Map;
use crate::palette::Palette;

pub const RHOMB_WIDTH: i32 = 64;
pub const RHOMB_HEIGHT: i32 = 32;
pub const RHOMB_BYTES: usize = 1024;

/// Width of each of the 32 rows of a rhomb: 3, 7, ..., 63, 61, ..., 1.
pub const fn row_width(y: i32) -> i32 {
    if y < 16 { 4 * y + 3 } else { 4 * (31 - y) + 1 }
}

/// Offset of a row's first pixel from the top vertex's column.
pub const fn row_start(y: i32) -> i32 {
    if y < 16 { -1 - 2 * y } else { -30 + 2 * (y - 16) }
}

/// Cells this close to the map edge are drawn black.
pub const BORDER: u32 = 4;

/// A rectangle in screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Brightness level (8.8 fixed point) and checkerboard bit of each pixel of a rhomb, in row order,
/// for corner levels `top` (the cell), `right` (x + 1), `left` (y + 1) and `bottom` (x + 1, y + 1):
/// `cad800.dll` `0x10003430`.
pub fn shade(top: u8, right: u8, left: u8, bottom: u8, phase: bool) -> impl Iterator<Item = (usize, bool)> {
    let (top, right, left, bottom) = (top as i32, right as i32, left as i32, bottom as i32);
    let step = (right - left) << 2;
    (0..RHOMB_HEIGHT).flat_map(move |row| {
        let start = if row < 16 {
            (top << 8) + (row + 1) * ((left - top) << 4) + step
        } else {
            (left << 8) + (row - 15) * ((bottom - left) << 4)
        };
        let flip = phase ^ (row >= 16) ^ (row & 1 == 1);
        (0..row_width(row)).map(move |i| ((((start + i * step) >> 8) & 0xff) as usize, flip ^ (i & 1 == 1)))
    })
}

/// Rasterise the ground under the screen rectangle `rect` to RGBA8. Pixels outside the map stay transparent.
pub fn rasterize_ground(map: &Map, rhombs: &[u8], palettes: &[Palette], rect: Rect) -> Vec<u8> {
    let mut out = vec![0u8; (rect.w * rect.h * 4) as usize];
    if palettes.is_empty() {
        return out;
    }
    let proj = Projection::of(map);
    let (w, h) = (map.width, map.height);
    let empty = [0u8; RHOMB_BYTES];
    // cells whose top vertex (d * 32 + offset, s * 16), with d = cx - cy, s = cx + cy, can touch the rectangle
    let (top_x0, _) = proj.cell_centre(0, 0);
    let d0 = (rect.x - RHOMB_WIDTH - top_x0).div_euclid(32);
    let d1 = (rect.x + rect.w + RHOMB_WIDTH - top_x0).div_euclid(32);
    let s0 = (rect.y - RHOMB_HEIGHT).div_euclid(16).max(0);
    let s1 = (rect.y + rect.h).div_euclid(16);
    for s in s0..=s1 {
        for d in d0..=d1 {
            if (s + d).rem_euclid(2) != 0 {
                continue;
            }
            let (cx, cy) = ((s + d) / 2, (s - d) / 2);
            if cx < 0 || cy < 0 || cx >= w as i32 || cy >= h as i32 {
                continue;
            }
            let (cx, cy) = (cx as u32, cy as u32);
            let inner = cx >= BORDER && cy >= BORDER && cx < w - BORDER && cy < h - BORDER;
            let level = |x: u32, y: u32| map.rhomb(x.min(w - 1), y.min(h - 1)).1 & 31;
            let (pixels, corners) = if inner {
                let start = map.rhomb(cx, cy).0 as usize * RHOMB_BYTES;
                let Some(pixels) = rhombs.get(start..start + RHOMB_BYTES) else { continue };
                (pixels, [level(cx, cy), level(cx + 1, cy), level(cx, cy + 1), level(cx + 1, cy + 1)])
            } else {
                (&empty[..], [0; 4])
            };
            let (top_x, centre_y) = proj.cell_centre(cx as i32, cy as i32);
            let top_y = centre_y - RHOMB_HEIGHT / 2;
            let mut shades = shade(corners[0], corners[1], corners[2], corners[3], cx & 1 == 1);
            let mut p = 0;
            for row in 0..RHOMB_HEIGHT {
                let y = top_y + row - rect.y;
                let x_start = top_x + row_start(row) - rect.x;
                for i in 0..row_width(row) {
                    let (lvl, odd) = shades.next().unwrap_or((0, false));
                    let x = x_start + i;
                    if (0..rect.h).contains(&y) && (0..rect.w).contains(&x) {
                        let palette = &palettes[(lvl | if odd { 32 } else { 0 }) % palettes.len()];
                        let c = palette.colours[pixels[p] as usize];
                        out[((y * rect.w + x) * 4) as usize..][..4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                    }
                    p += 1;
                }
            }
        }
    }
    out
}
