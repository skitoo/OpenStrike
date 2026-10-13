//! The map as the game's movement code sees it: one 32-bit word per 32 px cell, in a fixed
//! 256 x 256 array (`Game_Dll.dll` `0x101054e6`, row stride 256).
//!
//! Bits 0-3 block the four 16 px quarters of the cell (bit `qx + 2 * qy`), bit 14 keeps vehicles
//! out, bits 20-23 mark mines known to each player and bit 30 marks the border. The loader
//! (`0x1004c980`) copies the map's per-cell words, then forces the 4 outermost cells on each side
//! to `0x4000000f` (all quarters blocked, border bit) after clearing bits 13-29.

pub const SIDE: i32 = 256;
pub const CELLS: usize = (SIDE * SIDE) as usize;
/// Cell side in map pixels and in 1/256 pixel.
pub const CELL_PX: i32 = 32;
pub const CELL_SUB: i32 = CELL_PX * 256;

pub const QUARTERS: u32 = 0xf;
pub const NO_VEHICLES: u32 = 0x4000;
pub const BORDER: u32 = 0x4000_0000;
/// Width of the blocked border, in cells.
pub const BORDER_CELLS: i32 = 4;

/// The 8 neighbour steps then "stay" (`0x100b2144`): 0 east, clockwise with y down.
pub const DIRS: [(i32, i32); 9] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1), (0, 0)];
pub const STOP: u8 = 8;

/// Direction from the signs of a step (`0x100b2130`, indexed by `sx * 3 + sy`).
pub fn dir_of_signs(sx: i32, sy: i32) -> u8 {
    const T: [u8; 9] = [5, 4, 3, 6, 8, 2, 7, 0, 1];
    T[(sx.signum() * 3 + sy.signum() + 4) as usize]
}

/// For a diagonal step `dir`, the quarters of the two side cells that must be free: the cell
/// beside the target in x (`(nx, y)`, `0x100b218c`) and in y (`(x, ny)`, `0x100b2190`).
pub const SIDE_X_QUARTERS: [u32; 4] = [4, 8, 2, 1];
pub const SIDE_Y_QUARTERS: [u32; 4] = [2, 1, 4, 8];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    /// Size of the map in cells (at most 256 x 256).
    pub width: i32,
    pub height: i32,
    flags: Vec<u32>,
}

impl Grid {
    /// Build from the per-cell words of the map (`width * height`, row by row).
    pub fn from_map(width: i32, height: i32, cells: &[u32]) -> Self {
        assert!(width <= SIDE && height <= SIDE, "maps are at most 256 x 256 cells");
        let mut flags = vec![0u32; CELLS];
        for y in 0..height {
            for x in 0..width {
                // bits 19-29 come from later sections of the map (mines, zones) not read yet
                flags[(y * SIDE + x) as usize] = cells[(y * width + x) as usize] & !0x3ff8_0000;
            }
        }
        for y in 0..height {
            for x in 0..width {
                let edge = x < BORDER_CELLS || y < BORDER_CELLS || x >= width - BORDER_CELLS || y >= height - BORDER_CELLS;
                if edge {
                    let f = &mut flags[(y * SIDE + x) as usize];
                    *f = (*f & 0xc000_1fff) | BORDER | QUARTERS;
                }
            }
        }
        Self { width, height, flags }
    }

    /// An open map: only the border is blocked.
    pub fn open(width: i32, height: i32) -> Self {
        Self::from_map(width, height, &vec![0; (width * height) as usize])
    }

    /// Block the given quarters of a cell (for tests and editors).
    pub fn with_blocked(mut self, cells: impl IntoIterator<Item = (i32, i32)>, quarters: u32) -> Self {
        for (x, y) in cells {
            if let Some(i) = index(x, y) {
                self.flags[i] |= quarters;
            }
        }
        self
    }

    /// Word at a linear index of the 256 x 256 array; outside the array reads as a blocked border.
    pub fn at_linear(&self, i: i32) -> u32 {
        if (0..CELLS as i32).contains(&i) { self.flags[i as usize] } else { BORDER | QUARTERS }
    }

    pub fn at(&self, x: i32, y: i32) -> u32 {
        self.at_linear(y * SIDE + x)
    }

    /// Whether a 16 px quarter is blocked, for display.
    pub fn quarter_blocked(&self, qx: i32, qy: i32) -> bool {
        self.at(qx >> 1, qy >> 1) >> ((qx & 1) + 2 * (qy & 1)) & 1 != 0
    }
}

/// Linear index of a cell, if inside the array.
pub fn index(x: i32, y: i32) -> Option<usize> {
    let i = y * SIDE + x;
    (0..CELLS as i32).contains(&i).then_some(i as usize)
}
