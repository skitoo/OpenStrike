//! The game's isometric projection.
//!
//! Map space: square cells of 32x32 map pixels (`map::CELL`), where objects and units live.
//! Screen space: map pixel (x, y) is drawn at (x - y, (x + y) / 2), so a cell becomes a 64x32 rhomb.
//! Inferred from the data: rail pieces placed at a constant x every 80 map pixels only join up
//! under this projection, and it is the one that maps a cell exactly onto a rhomb.
//!
//! Screen coordinates here are shifted right by `height * 32` so that the whole map is at x >= 0.

use crate::map::{CELL, Map};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Projection {
    /// Map size in cells.
    pub width: u32,
    pub height: u32,
}

impl Projection {
    pub fn of(map: &Map) -> Self {
        Self { width: map.width, height: map.height }
    }

    fn offset_x(&self) -> i32 {
        (self.height * CELL) as i32
    }

    /// Screen position of a map pixel.
    pub fn to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        (x - y + self.offset_x(), (x + y).div_euclid(2))
    }

    /// Map position under a screen point (the inverse of `to_screen`, in floating point for input).
    pub fn to_map(&self, sx: f32, sy: f32) -> (f32, f32) {
        let sx = sx - self.offset_x() as f32;
        (sy + sx / 2.0, sy - sx / 2.0)
    }

    /// Screen size of the bounding box of the whole map.
    pub fn screen_size(&self) -> (i32, i32) {
        let (w, h) = ((self.width * CELL) as i32, (self.height * CELL) as i32);
        (w + h, (w + h) / 2)
    }

    /// Screen centre of a cell's rhomb.
    pub fn cell_centre(&self, cx: i32, cy: i32) -> (i32, i32) {
        let half = CELL as i32 / 2;
        self.to_screen(cx * CELL as i32 + half, cy * CELL as i32 + half)
    }
}
