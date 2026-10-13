//! Static passability of the map, on a grid of `TILE` map pixels.

use crate::{SUBPIXEL, Vec2};

/// Map pixels per terrain tile (half a map cell, the resolution of the building footprints).
pub const TILE: i32 = 16;

/// Which tiles ground vehicles can enter. Built once per map by the client; never changes during a game yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terrain {
    pub width: i32,
    pub height: i32,
    blocked: Vec<bool>,
}

impl Terrain {
    /// An open terrain of `width` x `height` tiles.
    pub fn open(width: i32, height: i32) -> Self {
        Self { width, height, blocked: vec![false; (width * height).max(0) as usize] }
    }

    pub fn with_blocked(mut self, tiles: impl IntoIterator<Item = (i32, i32)>) -> Self {
        for (x, y) in tiles {
            if self.contains(x, y) {
                let i = self.index(x, y);
                self.blocked[i] = true;
            }
        }
        self
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub(crate) fn index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    /// False outside the terrain.
    pub fn passable(&self, x: i32, y: i32) -> bool {
        self.contains(x, y) && !self.blocked[self.index(x, y)]
    }

    /// Tile under a position in sub-units.
    pub fn tile_of(p: Vec2) -> (i32, i32) {
        (p.x.div_euclid(TILE * SUBPIXEL), p.y.div_euclid(TILE * SUBPIXEL))
    }

    /// Centre of a tile, in sub-units.
    pub fn centre(x: i32, y: i32) -> Vec2 {
        Vec2::new(x * TILE * SUBPIXEL + TILE * SUBPIXEL / 2, y * TILE * SUBPIXEL + TILE * SUBPIXEL / 2)
    }
}
