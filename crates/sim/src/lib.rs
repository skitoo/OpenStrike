//! Deterministic game simulation.
//!
//! The whole game state is a value ([`World`]) and advances only through [`step`], at a fixed rate,
//! from the commands issued during the tick. Movement reproduces the original game
//! (`Game_Dll.dll`, addresses in the comments and in `docs/movement.md`): units live on 32 px cells,
//! drive along the curves of `CURVENEW` from cell to cell, turn in place one heading at a time, and
//! follow paths of cell directions planned by the game's own path finder.

pub mod fmath;
pub mod grid;
mod motion;
mod path;

pub use grid::Grid;

/// Simulation ticks per second: the single-player default tick is 40 ms (`0x10090ffa`).
pub const TICKS_PER_SECOND: u32 = 25;

/// Sub-units per map pixel.
pub const SUBPIXEL: i32 = 256;

/// Crew condition of a full, unhurt crew (`unit + 0x3d`, 0 to `0x8000`): scales speed and delays.
pub const CREW_FULL: i32 = 0x8000;

/// `path.cost` of a path not planned yet (`0x7fffff`).
pub(crate) const FRESH_COST: i32 = 0x7f_ffff;

/// The game's random generator (`0x10299130`), shared by every system.
pub fn lcg(x: u32) -> u32 {
    x.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId(pub u32);

/// Movement parameters of a unit type, from its description (`Game_Dll.dll` `0x100479c0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitType {
    pub name: String,
    /// `MoveSpeed a,b`: speed in 1/256 px per tick is `min + range * crew / 0x8000`.
    pub move_min: i32,
    pub move_range: i32,
    /// `TurnDelay a,b`: ticks between two heading steps, `min + range * crew / 0x8000`.
    pub turn_min: i32,
    pub turn_range: i32,
    /// `GunTurnDelay`: ticks between two turret heading steps.
    pub gun_turn_delay: i32,
    /// `IgnoreMines`.
    pub ignore_mines: bool,
    /// `CanMoveBackward`: may drive backwards when the next cell is behind.
    pub can_move_backward: bool,
    /// `Health`: hit points when unhurt (`+0xb8`, 1 by default, `0x10046344`).
    pub health: i32,
    /// `Ammo1`, `Ammo2`: rounds of each kind when full (`+0xbc`, `+0xc0`).
    pub ammo: [i32; 2],
    /// `MoveDamageHP`: hit points under which a vehicle is immobilised (`+0xb47`).
    pub move_damage_hp: i32,
}

impl UnitType {
    /// From the description values: `MoveSpeed` floats are scaled by 256 and truncated, the second
    /// value stored as a difference (`0x10047aa7`); `TurnDelay` likewise without scaling.
    pub fn new(name: &str, move_speed: (f32, f32), turn_delay: (i32, i32), gun_turn_delay: i32) -> Self {
        let min = (move_speed.0 as f64 * 256.0) as i32;
        let max = (move_speed.1 as f64 * 256.0) as i32;
        Self {
            name: name.to_string(),
            move_min: min,
            move_range: max - min,
            turn_min: turn_delay.0,
            turn_range: turn_delay.1 - turn_delay.0,
            gun_turn_delay,
            ignore_mines: false,
            can_move_backward: false,
            health: 1,
            ammo: [0, 0],
            move_damage_hp: 0,
        }
    }

    /// 1/256 px per tick (`0x100142e0`, without a towed gun).
    pub fn speed(&self, crew: i32) -> i32 {
        self.move_min + ((self.move_range * crew) >> 15)
    }

    pub fn turn_delay(&self, crew: i32) -> i32 {
        self.turn_min + ((self.turn_range * crew) >> 15)
    }
}

/// Static data of a game: the map, the curves and the unit types.
#[derive(Debug, Clone)]
pub struct Rules {
    pub grid: Grid,
    /// `CURVENEW` points `(x, y)` in 1/256 px from the cell corner, per `next * 9 + previous`.
    pub curves: Vec<Vec<(i16, i16)>>,
    pub types: Vec<UnitType>,
}

impl Rules {
    pub(crate) fn curve(&self, next: u8, previous: u8) -> &[(i16, i16)] {
        &self.curves[next as usize * 9 + previous as usize]
    }
}

/// State of a unit's path (`unit + 0x6c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PathStatus {
    #[default]
    None,
    /// Waiting for the path finder.
    Wanted,
    /// The order cannot be carried out.
    Failed,
    Ready,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathState {
    pub status: PathStatus,
    /// Ticks spent waiting behind other units, raises the planning order (`+0x71`).
    pub blocked_ticks: u16,
    /// Length of the first plan, ignoring units (`+0x73`).
    pub cost: i32,
    /// Progress per cell of the first plan (`+0x77`).
    pub ratio: f64,
    /// Ring of 64 cell directions from `index` to `end`.
    pub index: u8,
    pub end: u8,
    pub dirs: [u8; 64],
    /// Failed plans; 150 gives the order up (`+0xa2`).
    pub retries: u8,
    /// Ticks before the next plan (`+0xa3`).
    pub wait: u8,
    pub replans: i32,
}

impl Default for PathState {
    fn default() -> Self {
        Self {
            status: PathStatus::None,
            blocked_ticks: 0,
            cost: FRESH_COST,
            ratio: 0.0,
            index: 0,
            end: 0,
            dirs: [0; 64],
            retries: 0,
            wait: 0,
            replans: 0,
        }
    }
}

impl PathState {
    pub fn next(&self) -> Option<u8> {
        (self.index != self.end).then(|| self.dirs[self.index as usize])
    }

    pub(crate) fn clear(&mut self) {
        self.index = 0;
        self.end = 0;
    }

    /// The remaining directions, in order.
    pub fn remaining(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = self.index;
        while i != self.end {
            out.push(self.dirs[i as usize]);
            i = (i + 1) % 64;
        }
        out
    }
}

/// Cells a unit holds in the occupancy table (`unit + 0x66`): the cell it is in and, while moving
/// in, the one it comes from (`0x1002c7a0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reservation {
    pub x: i16,
    pub y: i16,
    /// Direction it entered from, or [`grid::STOP`].
    pub dir: u8,
}

/// Position on a curve (`unit + 0xb4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveState {
    pub previous: u8,
    pub next: u8,
    pub point: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Idle,
    /// Move to a cell (order 2, `0x1002d290`).
    Move { x: i16, y: i16 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub id: UnitId,
    /// Index into [`Rules::types`].
    pub kind: usize,
    pub player: u8,
    /// Crew condition, [`CREW_FULL`] when unhurt.
    pub crew: i32,
    /// Hit points (`+0x37`).
    pub hp: i32,
    /// Rounds left of each kind (`+0x39`, `+0x3b`).
    pub ammo: [i32; 2],
    pub cell: (i16, i16),
    /// Offset from the cell's top-left corner, 1/256 px; may leave the cell while driving out.
    pub sub: (i16, i16),
    /// Hull heading, 0-31 clockwise from east (map axes).
    pub facing: u8,
    pub desired: u8,
    pub turn_wait: u8,
    pub turret: u8,
    pub turret_wait: u8,
    pub curve: CurveState,
    /// Curve point being driven to, 1/256 px from the cell corner.
    pub aim: (i16, i16),
    pub reverse: bool,
    pub reservation: Reservation,
    pub path: PathState,
    pub order: Order,
    /// Cell the movement code heads for (`+0xa8`, `+0xa9`).
    pub goal: Option<(i16, i16)>,
    pub moving: bool,
}

impl Unit {
    /// A unit standing still in the middle of a cell (the stop point of the curves).
    pub fn new(id: UnitId, kind: usize, cell: (i16, i16), facing: u8) -> Self {
        let centre = (0xf00, 0xf00);
        Self {
            id,
            kind,
            player: 0,
            crew: CREW_FULL,
            hp: 1,
            ammo: [0, 0],
            cell,
            sub: centre,
            facing,
            desired: facing,
            turn_wait: 0,
            turret: facing,
            turret_wait: 0,
            curve: CurveState { previous: grid::STOP, next: grid::STOP, point: 0 },
            aim: centre,
            reverse: false,
            reservation: Reservation { x: cell.0, y: cell.1, dir: grid::STOP },
            path: PathState::default(),
            order: Order::Idle,
            goal: None,
            moving: false,
        }
    }

    /// Position in 1/256 map pixel.
    pub fn position(&self) -> (i32, i32) {
        (self.cell.0 as i32 * grid::CELL_SUB + self.sub.0 as i32, self.cell.1 as i32 * grid::CELL_SUB + self.sub.1 as i32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Move units to a cell, each to its own place around it (see [`group_targets`]).
    Move { units: Vec<UnitId>, to: (i16, i16) },
}

/// Whether a group order may send a unit to a cell (`0x1003e870`, vehicles): inside the border,
/// all quarters open, no other unit in it now, no known mine.
fn order_cell_ok(rules: &Rules, occupancy: &[u32], ty: &UnitType, x: i32, y: i32) -> bool {
    let g = &rules.grid;
    if x <= 3 || y <= 3 || x >= g.width - 4 || y >= g.height - 4 {
        return false;
    }
    let f = g.at(x, y);
    if f & grid::QUARTERS != 0 || f & grid::NO_VEHICLES != 0 {
        return false;
    }
    if grid::index(x, y).is_some_and(|i| occupancy[i] & 0xffff != 0) {
        return false;
    }
    let known_mines = 0u32;
    ty.ignore_mines || known_mines & (f >> 20) & 0xf == 0
}

/// Where each unit of a group order goes (`0x100609c0` with `0x1005f1e0`, `0x1005f230`,
/// `0x1005f290`, `0x1005f320`). A group spanning less than 21 cells keeps its shape around the
/// target; otherwise, or when a unit's place is not allowed, the unit takes the first free slot of
/// a spiral around the target in a 32 x 32 grid, every other row and column left empty for a spread
/// group. A vehicle fills a slot (infantry would share it four to a cell). `units` are in the order
/// of the selection list.
pub fn group_targets(rules: &Rules, occupancy: &[u32], units: &[&Unit], to: (i16, i16)) -> Vec<(i16, i16)> {
    let n = units.len() as i32;
    if n == 0 {
        return Vec::new();
    }
    let (tx, ty) = (to.0 as i32, to.1 as i32);
    let (mut sx, mut sy) = (0i32, 0i32);
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (1_000_000, -1_000_000, 1_000_000, -1_000_000);
    for u in units {
        let (x, y) = (u.cell.0 as i32, u.cell.1 as i32);
        sx += x;
        sy += y;
        (min_x, max_x, min_y, max_y) = (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y));
    }
    let (cx, cy) = (sx / n, sy / n);
    let spread = {
        let (a, b) = ((min_x - max_x).abs(), (min_y - max_y).abs());
        if b < a { b / 2 + a } else { a / 2 + b }
    };
    let keep_shape = spread < 0x15;
    let mut slots = [[0u8; 32]; 32];
    if !keep_shape {
        for (v, row) in slots.iter_mut().enumerate() {
            for (u, slot) in row.iter_mut().enumerate() {
                if (u | v) & 1 != 0 {
                    *slot = (*slot & 0x84) | 4;
                }
            }
        }
    }
    units
        .iter()
        .map(|u| {
            let t = &rules.types[u.kind];
            let ok = |x: i32, y: i32| order_cell_ok(rules, occupancy, t, x, y);
            if keep_shape {
                let (x, y) = (u.cell.0 as i32 + tx - cx, ty + u.cell.1 as i32 - cy);
                if ok(x, y) {
                    return (x as i16, y as i16);
                }
            }
            let (x, y) = spiral_slot(&mut slots, &ok, tx, ty, 4);
            (x as i16, y as i16)
        })
        .collect()
}

/// First slot around the target with room for `size` (4 fills it), ring by ring: top row left to
/// right, left column downwards, bottom row right to left, right column upwards. A side only moves
/// on past cells the unit may go to, as in the game.
fn spiral_slot(slots: &mut [[u8; 32]; 32], ok: &dyn Fn(i32, i32) -> bool, tx: i32, ty: i32, size: u8) -> (i32, i32) {
    let fits = |slot: u8| (slot & 0x7f).wrapping_add(size) < 5;
    let take = |slots: &mut [[u8; 32]; 32], x: i32, y: i32| {
        let s = &mut slots[y as usize][x as usize];
        *s = (*s & 0x80) | (s.wrapping_add(size) & 0x7f) | 0x80;
        (x - 15 + tx, y - 15 + ty)
    };
    let (mut low, mut high, mut n) = (15i32, 15i32, 1i32);
    while low > 0 {
        let mut x = low;
        for _ in 0..=n {
            if ok(x - 15 + tx, low - 15 + ty) {
                if fits(slots[low as usize][x as usize]) {
                    return take(slots, x, low);
                }
                x += 1;
            }
        }
        let mut y = low;
        for _ in 0..n {
            if ok(low - 15 + tx, y - 15 + ty) {
                if fits(slots[y as usize][low as usize]) {
                    return take(slots, low, y);
                }
                y += 1;
            }
        }
        let mut x = high;
        for _ in 0..=n {
            if ok(x - 15 + tx, high - 15 + ty) {
                if fits(slots[high as usize][x as usize]) {
                    return take(slots, x, high);
                }
                x -= 1;
            }
        }
        let mut y = high;
        for _ in 0..=n {
            if ok(high - 15 + tx, y - 15 + ty) {
                if fits(slots[y as usize][high as usize]) {
                    return take(slots, high, y);
                }
                y -= 1;
            }
        }
        low -= 1;
        high += 1;
        n += 2;
    }
    (tx, ty)
}

#[derive(Debug, Clone, PartialEq)]
pub struct World {
    pub tick: u64,
    /// The game's random generator state.
    pub rng: u32,
    /// In the order the game updates them.
    pub units: Vec<Unit>,
    /// Unit occupancy per cell (`0x101454e6`).
    pub occupancy: Vec<u32>,
}

impl World {
    pub fn new(units: Vec<Unit>, seed: u32) -> Self {
        let mut world = Self { tick: 0, rng: seed, units: Vec::new(), occupancy: vec![0; grid::CELLS] };
        for u in units {
            motion::occupy(&mut world.occupancy, u.reservation, 1);
            world.units.push(u);
        }
        world
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.iter().find(|u| u.id == id)
    }
}

/// Give a move order (`0x1002d290`): a new goal resets the path.
fn order_move(unit: &mut Unit, to: (i16, i16)) {
    unit.order = Order::Move { x: to.0, y: to.1 };
    if unit.goal != Some(to) {
        unit.goal = Some(to);
        unit.path = PathState::default();
    }
}

/// End the movement (`0x1002d200`).
fn stop_moving(unit: &mut Unit) {
    unit.goal = None;
    unit.path = PathState::default();
}

/// Advance the world by one tick.
pub fn step(world: &World, rules: &Rules, commands: &[Command]) -> World {
    let mut w = world.clone();
    for Command::Move { units, to } in commands {
        // the selection list is built by prepending while walking the unit list (`0x10068900`)
        let group: Vec<usize> = (0..w.units.len()).rev().filter(|&i| units.contains(&w.units[i].id)).collect();
        let members: Vec<&Unit> = group.iter().map(|&i| &w.units[i]).collect();
        let places = group_targets(rules, &w.occupancy, &members, *to);
        for (i, place) in group.into_iter().zip(places) {
            order_move(&mut w.units[i], place);
        }
    }
    for i in 0..w.units.len() {
        let mut unit = w.units[i].clone();
        update(&mut unit, rules, &mut w.occupancy, &mut w.rng);
        w.units[i] = unit;
    }
    schedule_paths(&mut w, rules);
    w.tick += 1;
    w
}

/// One unit's tick, as far as movement goes (`0x10014c00` and `0x10010b20`).
fn update(unit: &mut Unit, rules: &Rules, occupancy: &mut [u32], rng: &mut u32) {
    unit.path.wait = unit.path.wait.saturating_sub(1);
    let status = motion::drive(unit, rules, occupancy, rng);
    motion::turn_turret(unit, rules);
    if let Order::Move { .. } = unit.order
        && matches!(status, motion::Status::Arrived | motion::Status::GaveUp)
    {
        stop_moving(unit);
        unit.order = Order::Idle;
    }
}

/// Order in which the scheduler handles a unit, and marks it for the others (`0x1002da60`).
fn mark_order(unit: &Unit, rules: &Rules) -> usize {
    if unit.goal.is_none() {
        return 0;
    }
    let speed_bucket = (rules.types[unit.kind].speed(unit.crew) >> 3) as usize;
    if unit.reservation.dir == grid::STOP {
        match unit.path.status {
            PathStatus::Wanted if unit.path.wait == 0 => speed_bucket,
            PathStatus::Ready => speed_bucket,
            _ => 0,
        }
    } else {
        speed_bucket
    }
    .min(255)
}

/// Plan the paths of the units that want one, slowest first; each unit is marked as an obstacle
/// for the following ones once its turn has passed (`0x100405c0`).
fn schedule_paths(w: &mut World, rules: &Rules) {
    let mut to_plan: Vec<Vec<usize>> = vec![Vec::new(); 256];
    let mut to_mark: Vec<Vec<usize>> = vec![Vec::new(); 256];
    for (i, u) in w.units.iter_mut().enumerate() {
        let order = mark_order(u, rules);
        to_mark[order].insert(0, i);
        if u.path.status == PathStatus::Wanted {
            if u.path.wait == 0 {
                let priority = (order + (u.path.blocked_ticks >> 4) as usize).min(255);
                to_plan[priority].insert(0, i);
            } else {
                u.path.clear();
            }
        }
    }
    let mut marks = vec![0u8; grid::CELLS];
    for order in 0..256 {
        for &i in &to_plan[order] {
            let unit = &mut w.units[i];
            let Some(goal) = unit.goal else {
                unit.path.clear();
                unit.path.status = PathStatus::Failed;
                continue;
            };
            let ty = &rules.types[unit.kind];
            let mut env = path::Env {
                grid: &rules.grid,
                occupancy: &w.occupancy,
                marks: &marks,
                own: grid::index(unit.cell.0 as i32, unit.cell.1 as i32),
                avoid_units: false,
                ignore_mines: ty.ignore_mines,
                known_mines: 0,
            };
            let from = (unit.cell.0 as i32, unit.cell.1 as i32);
            let ok = path::plan(&mut env, &mut unit.path, &mut w.rng, from, (goal.0 as i32, goal.1 as i32));
            if ok {
                // the acknowledgement voice draws a number (`0x1004041a`)
                w.rng = lcg(w.rng);
            }
            if !ok {
                unit.path.clear();
                unit.path.status = PathStatus::Failed;
            } else if unit.path.index != unit.path.end {
                unit.path.status = PathStatus::Ready;
            }
        }
        for &i in &to_mark[order] {
            motion::mark(&mut marks, w.units[i].reservation);
        }
    }
}
