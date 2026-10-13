//! Deterministic A* over the terrain tiles: 8 directions, no corner cutting, integer costs,
//! ties broken by tile index so that every machine finds the same path.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::terrain::Terrain;

const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;
const NEIGHBOURS: [(i32, i32); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];

fn octile(a: (i32, i32), b: (i32, i32)) -> u32 {
    let (dx, dy) = ((a.0 - b.0).unsigned_abs(), (a.1 - b.1).unsigned_abs());
    STRAIGHT * dx.max(dy) + (DIAGONAL - STRAIGHT) * dx.min(dy)
}

/// Closest passable tile to `goal` (breadth-first, deterministic order), or `None` if there is none.
pub fn nearest_passable(terrain: &Terrain, goal: (i32, i32)) -> Option<(i32, i32)> {
    let clamp = (goal.0.clamp(0, terrain.width - 1), goal.1.clamp(0, terrain.height - 1));
    if terrain.passable(clamp.0, clamp.1) {
        return Some(clamp);
    }
    let mut seen = vec![false; (terrain.width * terrain.height) as usize];
    let mut queue = VecDeque::from([clamp]);
    seen[terrain.index(clamp.0, clamp.1)] = true;
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            if !terrain.contains(nx, ny) || seen[terrain.index(nx, ny)] {
                continue;
            }
            if terrain.passable(nx, ny) {
                return Some((nx, ny));
            }
            seen[terrain.index(nx, ny)] = true;
            queue.push_back((nx, ny));
        }
    }
    None
}

/// Tiles from `start` (excluded) to `goal` (included), or `None` when unreachable.
pub fn find(terrain: &Terrain, start: (i32, i32), goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    if !terrain.passable(goal.0, goal.1) || !terrain.contains(start.0, start.1) {
        return None;
    }
    if start == goal {
        return Some(Vec::new());
    }
    let n = (terrain.width * terrain.height) as usize;
    let mut cost = vec![u32::MAX; n];
    let mut came_from = vec![usize::MAX; n];
    let mut open = BinaryHeap::new();
    let s = terrain.index(start.0, start.1);
    cost[s] = 0;
    open.push(Reverse((octile(start, goal), s)));
    while let Some(Reverse((f, i))) = open.pop() {
        let (x, y) = ((i as i32) % terrain.width, (i as i32) / terrain.width);
        if f > cost[i] + octile((x, y), goal) {
            continue; // stale entry: this tile was reached more cheaply since
        }
        if (x, y) == goal {
            let mut path = Vec::new();
            let mut at = i;
            while at != s {
                path.push(((at as i32) % terrain.width, (at as i32) / terrain.width));
                at = came_from[at];
            }
            path.reverse();
            return Some(path);
        }
        for (dx, dy) in NEIGHBOURS {
            let (nx, ny) = (x + dx, y + dy);
            if !terrain.passable(nx, ny) {
                continue;
            }
            let diagonal = dx != 0 && dy != 0;
            if diagonal && !(terrain.passable(x + dx, y) && terrain.passable(x, y + dy)) {
                continue; // no squeezing between two blocked tiles
            }
            let j = terrain.index(nx, ny);
            let c = cost[i] + if diagonal { DIAGONAL } else { STRAIGHT };
            if c < cost[j] {
                cost[j] = c;
                came_from[j] = i;
                open.push(Reverse((c + octile((nx, ny), goal), j)));
            }
        }
    }
    None
}

/// Whether a straight line between two tile centres stays on passable tiles (supercover walk).
pub fn line_of_sight(terrain: &Terrain, a: (i32, i32), b: (i32, i32)) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (nx, ny) = (dx.abs(), dy.abs());
    let (sx, sy) = (dx.signum(), dy.signum());
    let (mut x, mut y) = a;
    let (mut ix, mut iy) = (0, 0);
    while ix < nx || iy < ny {
        // compare (ix + 0.5) / nx with (iy + 0.5) / ny in integers
        let decision = (1 + 2 * ix) * ny - (1 + 2 * iy) * nx;
        if decision == 0 {
            if !terrain.passable(x + sx, y) || !terrain.passable(x, y + sy) {
                return false;
            }
            x += sx;
            y += sy;
            ix += 1;
            iy += 1;
        } else if decision < 0 {
            x += sx;
            ix += 1;
        } else {
            y += sy;
            iy += 1;
        }
        if !terrain.passable(x, y) {
            return false;
        }
    }
    true
}

/// Drop the tiles that can be skipped in a straight line.
pub fn smooth(terrain: &Terrain, start: (i32, i32), path: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let mut from = start;
    let mut i = 0;
    while i < path.len() {
        let mut j = path.len() - 1;
        while j > i && !line_of_sight(terrain, from, path[j]) {
            j -= 1;
        }
        out.push(path[j]);
        from = path[j];
        i = j + 1;
    }
    out
}
