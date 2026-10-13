//! Integer geometry.

use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

impl Vec2 {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Euclidean length, rounded down.
    pub fn length(self) -> i64 {
        isqrt(self.x as i64 * self.x as i64 + self.y as i64 * self.y as i64)
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

pub fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = (n as f64).sqrt() as i64; // initial guess only; corrected below to the exact floor
    while x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

/// One of the game's 32 headings, clockwise from east in 11.25 degree steps (screen coordinates, y down).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Direction(pub u8);

/// Unit vectors of the 32 headings, scaled by 1024.
const HEADINGS: [(i64, i64); 32] = [
    (1024, 0), (1004, 200), (946, 392), (851, 569),
    (724, 724), (569, 851), (392, 946), (200, 1004),
    (0, 1024), (-200, 1004), (-392, 946), (-569, 851),
    (-724, 724), (-851, 569), (-946, 392), (-1004, 200),
    (-1024, 0), (-1004, -200), (-946, -392), (-851, -569),
    (-724, -724), (-569, -851), (-392, -946), (-200, -1004),
    (0, -1024), (200, -1004), (392, -946), (569, -851),
    (724, -724), (851, -569), (946, -392), (1004, -200),
];

impl Direction {
    pub const COUNT: u8 = 32;

    /// Closest heading to `v` (integer dot product, no trigonometry).
    pub fn towards(v: Vec2) -> Self {
        let best = (0..HEADINGS.len())
            .max_by_key(|&i| HEADINGS[i].0 * v.x as i64 + HEADINGS[i].1 * v.y as i64)
            .unwrap_or(0);
        Self(best as u8)
    }
}
