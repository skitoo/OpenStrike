//! The vehicles' path finder, reimplementing the behaviour of `Game_Dll.dll` (`0x10061e60` and what it calls).
//!
//! The search (`0x10061f90`) works on cells. It first labels the straight line to the goal
//! (diagonal steps first), then runs a 32-entry ring of walkers: a walker heads straight for the
//! goal; when blocked it splits into two wall followers (one turning each way) that go back to
//! heading for the goal once they reach a cell of the straight line further along. Every cell
//! reached gets its walking distance. Then, from the cell closest to the goal, it walks back to the
//! start: in each of the 8 directions it looks along a straight ray for the first reached cell and
//! steps one cell towards the ray with the smallest distance. The steps, reversed, are the path.
//!
//! The planner (`0x10067080`) runs the search twice, ignoring then avoiding the other units, and
//! keeps the second path only if it is not much worse; otherwise the unit waits and tries again.

use crate::fmath::sqrt;
use crate::grid::{
    self, BORDER, DIRS, Grid, NO_VEHICLES, QUARTERS, SIDE, SIDE_X_QUARTERS, SIDE_Y_QUARTERS, dir_of_signs,
};
use crate::{PathState, PathStatus, lcg};

/// Occupancy masks of the side cells for a diagonal step (`0x100b222c`, `0x100b2230`).
const SIDE_X_OCCUPIED: [u32; 4] = [0xf00, 0xf000, 0xf0, 0xf];
const SIDE_Y_OCCUPIED: [u32; 4] = [0xf0, 0xf, 0xf00, 0xf000];

/// Walkers with this walking distance or more are dropped.
const MAX_DIST: i32 = 0x3ff;
/// The path ring holds 64 directions; the search writes at most 61.
const PATH_RING: u8 = 64;
const PATH_WRITE_LIMIT: u8 = 0x3d;
/// Room of the back-tracking step table (`0x103410b0` up to the next global).
const MAX_BACK_STEPS: usize = 6342;

/// What the path code reads, with the globals it sets around a search.
pub(crate) struct Env<'a> {
    pub grid: &'a Grid,
    /// Unit occupancy per cell (`0x101454e6`): 4-bit counters per quarter.
    pub occupancy: &'a [u32],
    /// Cells of the units already handled this tick (`0x101e54e6`), seen only when `avoid_units`.
    pub marks: &'a [u8],
    /// The cell of the unit being routed reads as empty and passable (`0x10061e60`).
    pub own: Option<usize>,
    /// `0x1034d6ec`.
    pub avoid_units: bool,
    /// Type key `IgnoreMines` (`0x1034d6e0`).
    pub ignore_mines: bool,
    /// Mines known to the unit's player (`0x101ff3c8`), against bits 20-23 of the cells.
    pub known_mines: u32,
}

impl Env<'_> {
    fn flags_linear(&self, i: i32) -> u32 {
        let f = self.grid.at_linear(i);
        if self.own == Some(i as usize) { f & !(QUARTERS | NO_VEHICLES) } else { f }
    }

    fn flags(&self, x: i32, y: i32) -> u32 {
        self.flags_linear(y * SIDE + x)
    }

    fn mark(&self, x: i32, y: i32) -> u8 {
        match grid::index(x, y) {
            Some(i) if self.own == Some(i) => 0,
            Some(i) => self.marks[i],
            None => 0xff,
        }
    }

    fn occupied(&self, x: i32, y: i32) -> u32 {
        match grid::index(x, y) {
            Some(i) if self.own == Some(i) => 0,
            Some(i) => self.occupancy[i],
            None => u32::MAX,
        }
    }

    fn enterable(&self, f: u32) -> bool {
        f & QUARTERS == 0 && f & NO_VEHICLES == 0 && (self.ignore_mines || self.known_mines & (f >> 20) & 0xf == 0)
    }

    /// Step check of the search and of the planner's first callback (`0x100636b0`, `0x10061d80`).
    pub fn can_step(&self, x: i32, y: i32, nx: i32, ny: i32, dir: u8) -> bool {
        if self.avoid_units && self.mark(nx, ny) != 0 {
            return false;
        }
        if !self.enterable(self.flags(nx, ny)) {
            return false;
        }
        if dir & 1 != 0 {
            let k = (dir >> 1) as usize;
            if self.flags(nx, y) & SIDE_X_QUARTERS[k] & QUARTERS != 0
                || self.flags(x, ny) & SIDE_Y_QUARTERS[k] & QUARTERS != 0
            {
                return false;
            }
            if self.avoid_units
                && (self.mark(nx, y) & SIDE_X_QUARTERS[k] as u8 != 0
                    || self.mark(x, ny) & SIDE_Y_QUARTERS[k] as u8 != 0)
            {
                return false;
            }
        }
        true
    }

    /// Step check against the occupancy (`0x10061c90`).
    pub fn can_step_occupied(&self, x: i32, y: i32, nx: i32, ny: i32, dir: u8) -> bool {
        if self.avoid_units && self.occupied(nx, ny) & 0xffff != 0 {
            return false;
        }
        if !self.enterable(self.flags(nx, ny)) {
            return false;
        }
        if dir & 1 != 0 {
            let k = (dir >> 1) as usize;
            if self.flags(nx, y) & SIDE_X_QUARTERS[k] & QUARTERS != 0
                || self.flags(x, ny) & SIDE_Y_QUARTERS[k] & QUARTERS != 0
            {
                return false;
            }
            if self.avoid_units
                && (self.occupied(nx, y) & SIDE_X_OCCUPIED[k] != 0 || self.occupied(x, ny) & SIDE_Y_OCCUPIED[k] != 0)
            {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Walker {
    x: i32,
    y: i32,
    dir: u8,
    dist: i32,
    /// 0 heads for the goal, 1 and 2 follow a wall turning one way or the other.
    mode: u8,
    /// Straight-line label of the cell where the wall following started.
    label: i32,
}

/// Per-search tables (`0x1029dd88` to `0x10340d98`). Cells are indexed `x * 256 + y` here, as in
/// the game.
struct Tables {
    line: Vec<u8>,
    headed: Vec<u8>,
    dist: Vec<i16>,
    followed: Vec<u16>,
    /// Extent of the reached cells: x per row, y per column, x per diagonal and anti-diagonal.
    min_x_row: Vec<i32>,
    max_x_row: Vec<i32>,
    min_y_col: Vec<i32>,
    max_y_col: Vec<i32>,
    min_x_sum: Vec<i32>,
    max_x_sum: Vec<i32>,
    min_x_diff: Vec<i32>,
    max_x_diff: Vec<i32>,
}

/// Transposed index of the search tables; out of range wraps (the game would read neighbouring
/// memory; the border keeps walkers far from the edges).
fn t(x: i32, y: i32) -> usize {
    ((x * SIDE + y) as usize) & (grid::CELLS - 1)
}

impl Tables {
    fn new() -> Self {
        let cells = grid::CELLS;
        Self {
            line: vec![0; cells],
            headed: vec![0; cells],
            dist: vec![0; cells],
            followed: vec![0; cells],
            min_x_row: vec![0x7fff; 256],
            max_x_row: vec![0; 256],
            min_y_col: vec![0x7fff; 256],
            max_y_col: vec![0; 256],
            min_x_sum: vec![0x7fff; 512],
            max_x_sum: vec![0; 512],
            min_x_diff: vec![0x7fff; 512],
            max_x_diff: vec![0; 512],
        }
    }

    fn reach(&mut self, x: i32, y: i32) {
        let (row, col, sum, diff) =
            ((y & 255) as usize, (x & 255) as usize, ((x + y) & 511) as usize, ((x - y + 256) & 511) as usize);
        self.min_x_row[row] = self.min_x_row[row].min(x);
        self.max_x_row[row] = self.max_x_row[row].max(x);
        self.min_y_col[col] = self.min_y_col[col].min(y);
        self.max_y_col[col] = self.max_y_col[col].max(y);
        self.min_x_sum[sum] = self.min_x_sum[sum].min(x);
        self.max_x_sum[sum] = self.max_x_sum[sum].max(x);
        self.min_x_diff[diff] = self.min_x_diff[diff].min(x);
        self.max_x_diff[diff] = self.max_x_diff[diff].max(x);
    }
}

/// Result of a search (`0x10061f90` out parameter).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Found {
    /// Length of the path in cells, diagonals counting 1.4142135623.
    pub cost: f64,
    /// Squared distance from the closest reached cell to the goal.
    pub dist2: i32,
    pub x: i32,
    pub y: i32,
}

struct Ring {
    walkers: [Walker; 32],
    head: usize,
    tail: usize,
}

impl Ring {
    fn push_back(&mut self, w: Walker) {
        self.walkers[self.tail] = w;
        self.tail = (self.tail + 1) & 31;
    }

    fn push_front(&mut self, w: Walker) {
        self.head = (self.head + 31) & 31;
        self.walkers[self.head] = w;
    }

    fn pop(&mut self) -> Walker {
        let w = self.walkers[self.head];
        self.head = (self.head + 1) & 31;
        w
    }
}

/// Whether a walker may step from `(x, y)` in `dir`: the step check, and no diagonal across the
/// straight line.
fn walkable(env: &Env, tables: &Tables, x: i32, y: i32, dir: u8) -> bool {
    let (dx, dy) = DIRS[dir as usize];
    let (nx, ny) = (x + dx, y + dy);
    if !env.can_step(x, y, nx, ny, dir) {
        return false;
    }
    !(dir & 1 != 0 && tables.line[t(nx, y)] != 0 && tables.line[t(x, ny)] != 0)
}

/// Search from `(sx, sy)` to `(gx, gy)`, appending the path to `path` (`0x10061f90`).
pub(crate) fn search(env: &Env, path: &mut PathState, sx: i32, sy: i32, gx: i32, gy: i32) -> Found {
    let mut tables = Tables::new();
    // label the straight line, diagonal steps first
    tables.line[t(sx, sy)] = 1;
    let (mut x, mut y, mut label) = (sx, sy, 2u8);
    while (x, y) != (gx, gy) {
        let (dx, dy) = DIRS[dir_of_signs(gx - x, gy - y) as usize];
        x += dx;
        y += dy;
        tables.line[t(x, y)] = label;
        label = label.wrapping_add(1);
    }

    let mut ring = Ring { walkers: [Walker::default(); 32], head: 0, tail: 0 };
    let mut best_score = i32::MAX;
    let (mut bx, mut by) = (sx, sy);
    tables.dist[t(sx, sy)] = 2;
    tables.reach(sx, sy);
    ring.push_back(Walker {
        x: sx,
        y: sy,
        dir: dir_of_signs(gx - sx, gy - sy),
        dist: 2,
        mode: 0,
        label: tables.line[t(sx, sy)] as i32,
    });
    loop {
        let w = ring.pop();
        if w.dist < MAX_DIST {
            if (w.x, w.y) == (gx, gy) {
                (bx, by) = (gx, gy);
                break;
            }
            let score = (w.y - gy) * (w.y - gy) + (w.x - gx) * (w.x - gx) + w.dist / 4 + 0x10;
            if score < best_score {
                best_score = score;
                (bx, by) = (w.x, w.y);
            }
            if w.mode == 0 {
                head_for_goal(env, &mut tables, &mut ring, w, gx, gy);
            } else {
                follow_wall(env, &mut tables, &mut ring, w);
            }
        }
        if ring.tail == ring.head {
            break;
        }
    }

    // walk back from the closest cell to the start
    let mut steps: Vec<(i32, i32)> = Vec::new();
    let mut cost = 0.0f64;
    let (mut cx, mut cy) = (bx, by);
    while (cx, cy) != (sx, sy) && steps.len() < MAX_BACK_STEPS {
        let (dx, dy) = back_step(env, &tables, cx, cy);
        steps.push((dx, dy));
        let diagonal = (dx != 0 && dy != 0) as i32 as f64;
        cost = diagonal * 0.414_213_562_3 + cost + 1.0;
        cx += dx;
        cy += dy;
    }
    for &(dx, dy) in steps.iter().rev() {
        let next = (path.end + 1) % PATH_RING;
        if next != path.index {
            path.dirs[path.end as usize] = dir_of_signs(-dx, -dy);
            path.end = next;
        }
        if path.end >= PATH_WRITE_LIMIT {
            break;
        }
    }
    Found { cost, dist2: (by - gy) * (by - gy) + (bx - gx) * (bx - gx), x: bx, y: by }
}

fn head_for_goal(env: &Env, tables: &mut Tables, ring: &mut Ring, w: Walker, gx: i32, gy: i32) {
    if tables.headed[t(w.x, w.y)] != 0 {
        return;
    }
    tables.headed[t(w.x, w.y)] = 1;
    let dir = dir_of_signs(gx - w.x, gy - w.y);
    if walkable(env, tables, w.x, w.y, dir) {
        let (dx, dy) = DIRS[dir as usize];
        let (nx, ny) = (w.x + dx, w.y + dy);
        tables.reach(nx, ny);
        let dist = w.dist + 1;
        if tables.dist[t(nx, ny)] == 0 {
            tables.dist[t(nx, ny)] = dist as i16;
        }
        ring.push_back(Walker { x: nx, y: ny, dir, dist, mode: 0, label: w.label });
    } else {
        let label = tables.line[t(w.x, w.y)] as i32;
        ring.push_front(Walker { dir: (dir + 3) & 7, mode: 1, label, ..w });
        ring.push_front(Walker { dir: dir.wrapping_sub(3) & 7, mode: 2, label, ..w });
    }
}

fn follow_wall(env: &Env, tables: &mut Tables, ring: &mut Ring, w: Walker) {
    // mode 1 starts back towards the wall and turns clockwise, mode 2 the other way
    let (mut dir, bit, turn) =
        if w.mode == 1 { (w.dir.wrapping_sub(3) & 7, 0, 1u8) } else { ((w.dir + 3) & 7, 8, 7u8) };
    let (dx, dy) = DIRS[dir as usize];
    // the border test of this step reads the cell table transposed, as the game does
    if env.flags_linear((w.x + dx) * SIDE + (w.y + dy)) & BORDER != 0 {
        return;
    }
    let mask = 1u16 << (dir + bit);
    if tables.followed[t(w.x, w.y)] & mask != 0 {
        return;
    }
    tables.followed[t(w.x, w.y)] |= mask;
    let mut tries = 0;
    while tries < 10 {
        if walkable(env, tables, w.x, w.y, dir) {
            break;
        }
        dir = (dir + turn) & 7;
        tries += 1;
    }
    if tries >= 8 {
        return;
    }
    let (dx, dy) = DIRS[dir as usize];
    let (nx, ny) = (w.x + dx, w.y + dy);
    tables.reach(nx, ny);
    let dist = w.dist + 1;
    if tables.dist[t(nx, ny)] == 0 {
        tables.dist[t(nx, ny)] = dist as i16;
    }
    let mode = if tables.line[t(nx, ny)] as i32 > w.label { 0 } else { w.mode };
    ring.push_back(Walker { x: nx, y: ny, dir, dist, mode, label: w.label });
}

/// How far a back-tracking ray may go: the extent of the reached cells on its line.
#[derive(Clone, Copy)]
enum Bound {
    MinX(i32),
    MaxX(i32),
    MinY(i32),
    MaxY(i32),
}

impl Bound {
    fn within(self, x: i32, y: i32) -> bool {
        match self {
            Bound::MinX(b) => x >= b,
            Bound::MaxX(b) => x <= b,
            Bound::MinY(b) => y >= b,
            Bound::MaxY(b) => y <= b,
        }
    }
}

/// Walking distance of the first reached cell along a ray from `(x, y)`, or `0x7fff`.
fn ray(env: &Env, tables: &Tables, (x, y): (i32, i32), (dx, dy): (i32, i32), bound: Bound) -> i32 {
    let dir = dir_of_signs(dx, dy);
    let (mut x, mut y) = (x, y);
    if !bound.within(x, y) {
        return 0x7fff;
    }
    loop {
        if !env.can_step(x, y, x + dx, y + dy, dir) {
            return 0x7fff;
        }
        x += dx;
        y += dy;
        let d = tables.dist[t(x, y)];
        if d > 0 {
            return d as i32;
        }
        if !bound.within(x, y) {
            return 0x7fff;
        }
    }
}

/// One step back towards the start (`0x10062fa1`).
fn back_step(env: &Env, tables: &Tables, cx: i32, cy: i32) -> (i32, i32) {
    let diff = ((cx - cy + 256) & 511) as usize;
    let sum = ((cx + cy) & 511) as usize;
    let (row, col) = ((cy & 255) as usize, (cx & 255) as usize);
    let rays = [
        ((-1, -1), Bound::MinX(tables.min_x_diff[diff])),
        ((-1, 0), Bound::MinX(tables.min_x_row[row])),
        ((-1, 1), Bound::MinX(tables.min_x_sum[sum])),
        ((1, -1), Bound::MaxX(tables.max_x_sum[sum])),
        ((1, 0), Bound::MaxX(tables.max_x_row[row])),
        ((1, 1), Bound::MaxX(tables.max_x_diff[diff])),
        ((0, -1), Bound::MinY(tables.min_y_col[col])),
        ((0, 1), Bound::MaxY(tables.max_y_col[col])),
    ];
    // the first ray is taken whatever it finds; the others only when strictly closer
    let mut best = (0x7fff, (0, 0));
    for (i, (step, bound)) in rays.into_iter().enumerate() {
        let d = ray(env, tables, (cx, cy), step, bound);
        if i == 0 || d < best.0 {
            best = (d, step);
        }
    }
    best.1
}

/// Plan a path for a unit at `(cx, cy)` going to `(tx, ty)` (`0x10067080`). Returns false when the
/// order has to be given up.
pub(crate) fn plan(
    env: &mut Env,
    path: &mut PathState,
    rng: &mut u32,
    (cx, cy): (i32, i32),
    (tx, ty): (i32, i32),
) -> bool {
    *rng = lcg(*rng);
    let random = (*rng >> 16) as u8;
    env.avoid_units = true;
    let next = path.next();
    let straight = sqrt(((cy - ty) * (cy - ty) + (cx - tx) * (cx - tx)) as f64);
    if path.cost == crate::FRESH_COST {
        path.clear();
        env.avoid_units = false;
        let found = search(env, path, cx, cy, tx, ty);
        if path.index == path.end {
            path.ratio = 0.0;
            path.wait = 0;
            path.retries = 0;
            path.replans = 0;
            path.cost = crate::FRESH_COST;
            path.blocked_ticks = 0;
            path.status = PathStatus::Failed;
            return false;
        }
        path.cost = found.cost as i32;
        path.ratio =
            if path.cost != 0 { (straight - sqrt(found.dist2 as f64) + 6.0) / (found.cost + 6.0) } else { 0.0 };
    }
    match next {
        None => path.blocked_ticks = 0,
        Some(dir) => {
            let (dx, dy) = DIRS[dir as usize];
            if !env.can_step_occupied(cx, cy, cx + dx, cy + dy, dir) && env.can_step(cx, cy, cx + dx, cy + dy, dir) {
                // only another unit is in the way: keep the path and wait
                path.blocked_ticks = path.blocked_ticks.wrapping_add((random & 3) as u16);
                path.status = PathStatus::Wanted;
                return true;
            }
            path.blocked_ticks = 0;
        }
    }
    path.replans = path.replans.wrapping_add(1);
    path.clear();
    env.avoid_units = true;
    let found = search(env, path, cx, cy, tx, ty);
    env.avoid_units = false;
    let length_ratio = (path.cost as f64 + 32.0) / (found.cost + 32.0);
    let gain =
        if found.cost as i32 != 0 { (straight - sqrt(found.dist2 as f64) + 6.0) / (found.cost + 6.0) } else { 0.0 };
    if path.index != path.end && length_ratio >= 0.5 && gain >= path.ratio * 0.5 {
        path.status = PathStatus::Ready;
        path.retries = 0;
        path.wait = 0;
        return true;
    }
    env.avoid_units = true;
    let near = (cy - ty) * (cy - ty) + (cx - tx) * (cx - tx);
    if near < 0x100 && found.cost < 16.0 && (cx, cy) != (tx, ty) {
        // close to the goal: is the straight way clear of units?
        let (mut x, mut y, mut free, mut steps) = (cx, cy, 0, 0);
        while (x, y) != (tx, ty) {
            let (sx, sy) = ((tx - x).signum(), (ty - y).signum());
            if env.can_step_occupied(x, y, x + sx, y + sy, dir_of_signs(sx, sy)) {
                free += 1;
            }
            steps += 1;
            x += sx;
            y += sy;
        }
        if free * 5 / steps < 5 {
            path.retries = path.retries.wrapping_add(40);
        }
    }
    let retries = path.retries;
    path.wait = if retries < 0x14 { random & 3 } else { (random & 7) + 0x19 };
    path.retries = retries.wrapping_add(if length_ratio < 0.5 { 0xb } else { 1 });
    path.clear();
    if path.retries > 0x96 {
        path.status = PathStatus::Failed;
        return false;
    }
    path.status = PathStatus::Wanted;
    true
}
