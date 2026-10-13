//! Ground rhombs: `mappic.rs2` (in each climate archive) holds raw 8-bit 64x32 rhombs of 1024 bytes,
//! coloured with one of the 64 palettes of `rhomb.pl`. The palettes are brightness levels (0 dark,
//! 63 bright), selected per cell by the map.
//!
//! Loader in the original game: `Game_Dll.dll` `0x1004fac0`. Each cell is drawn as its rhomb under
//! the isometric projection (see [`crate::iso`]).

use crate::iso::Projection;
use crate::map::Map;
use crate::palette::Palette;

pub const RHOMB_WIDTH: i32 = 64;
pub const RHOMB_HEIGHT: i32 = 32;
pub const RHOMB_BYTES: usize = 1024;

/// Width of each of the 32 rows of a rhomb: 2, 6, ..., 62, 62, ..., 2.
pub const fn row_width(y: i32) -> i32 {
    if y < 16 { 4 * y + 2 } else { 4 * (31 - y) + 2 }
}

/// A rectangle in screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Rasterise the ground under the screen rectangle `rect` to RGBA8. Pixels outside the map stay transparent.
pub fn rasterize_ground(map: &Map, rhombs: &[u8], palettes: &[Palette], rect: Rect) -> Vec<u8> {
    let mut out = vec![0u8; (rect.w * rect.h * 4) as usize];
    if palettes.is_empty() {
        return out;
    }
    let proj = Projection::of(map);
    // cells whose rhomb centre (d * 32 + offset, s * 16 + 16) can touch the rectangle, with d = cx - cy, s = cx + cy
    let (centre_x0, _) = proj.cell_centre(0, 0);
    let d0 = (rect.x - RHOMB_WIDTH - centre_x0).div_euclid(32);
    let d1 = (rect.x + rect.w + RHOMB_WIDTH - centre_x0).div_euclid(32);
    let s0 = (rect.y - RHOMB_HEIGHT - 16).div_euclid(16).max(0);
    let s1 = (rect.y + rect.h + RHOMB_HEIGHT).div_euclid(16);
    for s in s0..=s1 {
        for d in d0..=d1 {
            if (s + d).rem_euclid(2) != 0 {
                continue;
            }
            let (cx, cy) = ((s + d) / 2, (s - d) / 2);
            if cx < 0 || cy < 0 || cx >= map.width as i32 || cy >= map.height as i32 {
                continue;
            }
            let (index, select) = map.rhomb(cx as u32, cy as u32);
            let start = index as usize * RHOMB_BYTES;
            let Some(pixels) = rhombs.get(start..start + RHOMB_BYTES) else { continue };
            let palette = &palettes[select as usize % palettes.len()];
            let (centre_x, centre_y) = proj.cell_centre(cx, cy);
            let mut p = 0;
            for row in 0..RHOMB_HEIGHT {
                let w = row_width(row);
                let y = centre_y - RHOMB_HEIGHT / 2 + row - rect.y;
                let x_start = centre_x - w / 2 - rect.x;
                if (0..rect.h).contains(&y) {
                    for i in 0..w {
                        let x = x_start + i;
                        if (0..rect.w).contains(&x) {
                            let c = palette.colours[pixels[p + i as usize] as usize];
                            out[((y * rect.w + x) * 4) as usize..][..4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                        }
                    }
                }
                p += w as usize;
            }
        }
    }
    out
}
