//! Ground rhombs: `<climate>/mappic.rs2` holds raw 8-bit 64x32 diamonds of 1024 bytes,
//! coloured with one of the 64 palettes of `<climate>/rhomb.pl`.
//!
//! Loader in the original game: `Game_Dll.dll` `0x1004fac0`.
//! The cell layout (diamond centred at `(32x, 32y + 16 * (x & 1))`) is inferred: it renders seamlessly
//! and matches the object coordinates, but has not been read from the original renderer yet.

use crate::map::Map;
use crate::palette::Palette;

pub const RHOMB_WIDTH: i32 = 64;
pub const RHOMB_HEIGHT: i32 = 32;
pub const RHOMB_BYTES: usize = 1024;

/// Width of each of the 32 rows of a rhomb: 2, 6, ..., 62, 62, ..., 2.
pub const fn row_width(y: i32) -> i32 {
    if y < 16 { 4 * y + 2 } else { 4 * (31 - y) + 2 }
}

/// Centre of a cell's rhomb, in map pixels.
pub const fn cell_centre(x: i32, y: i32) -> (i32, i32) {
    (32 * x, 32 * y + 16 * (x & 1))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Rasterise the ground of `rect` (map pixels) to RGBA8. Pixels outside the map stay transparent.
pub fn rasterize_ground(map: &Map, rhombs: &[u8], palettes: &[Palette], rect: Rect) -> Vec<u8> {
    let mut out = vec![0u8; (rect.w * rect.h * 4) as usize];
    if palettes.is_empty() {
        return out;
    }
    let (cx0, cx1) = ((rect.x / 32 - 1).max(0), ((rect.x + rect.w) / 32 + 2).min(map.width as i32));
    let (cy0, cy1) = ((rect.y / 32 - 1).max(0), ((rect.y + rect.h) / 32 + 2).min(map.height as i32));
    for cy in cy0..cy1 {
        for cx in cx0..cx1 {
            let (index, select) = map.rhomb(cx as u32, cy as u32);
            let start = index as usize * RHOMB_BYTES;
            let Some(pixels) = rhombs.get(start..start + RHOMB_BYTES) else { continue };
            let palette = &palettes[select as usize % palettes.len()];
            let (centre_x, centre_y) = cell_centre(cx, cy);
            let mut p = 0;
            for row in 0..RHOMB_HEIGHT {
                let w = row_width(row);
                let y = centre_y - RHOMB_HEIGHT / 2 + row - rect.y;
                let x_start = centre_x - w / 2 - rect.x;
                if (0..rect.h).contains(&y) {
                    for i in 0..w {
                        let x = x_start + i;
                        if (0..rect.w).contains(&x) {
                            let c = palette.0[pixels[p + i as usize] as usize];
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
