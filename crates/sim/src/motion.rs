//! Driving a unit along its path, one tick at a time (`Game_Dll.dll` `0x1002dee0`).

use crate::fmath::heading;
use crate::grid::{self, DIRS, NO_VEHICLES, QUARTERS, SIDE_X_QUARTERS, SIDE_Y_QUARTERS, STOP};
use crate::{CurveState, PathStatus, Reservation, Rules, Unit, lcg};

/// Occupancy added to the side cells of a diagonal move (`0x100b027c`, `0x100b0280`).
const SIDE_X_OCCUPANCY: [u32; 4] = [0x100, 0x1000, 0x10, 0x1];
const SIDE_Y_OCCUPANCY: [u32; 4] = [0x10, 0x1, 0x100, 0x1000];
/// Occupancy masks checked on the side cells of a diagonal step (`0x100b222c`, `0x100b2230`).
const SIDE_X_OCCUPIED: [u32; 4] = [0xf00, 0xf000, 0xf0, 0xf];
const SIDE_Y_OCCUPIED: [u32; 4] = [0xf0, 0xf, 0xf00, 0xf000];
/// A whole cell: every quarter counter of both halves (`0x11111111`).
const WHOLE_CELL: u32 = 0x1111_1111;
/// Curve points closer than this (max + min / 2, 1/256 px) count as reached.
const REACHED: i32 = 0xfa;

/// What the movement code reports (`0x1002dee0` return value, kept in `0x100c4510`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Moved,
    Turning,
    /// Stopped where the order wanted, or with nothing to do.
    Arrived,
    /// The path finder gave the order up.
    GaveUp,
    /// Stopped, waiting for a path.
    Waiting,
}

fn add(occupancy: &mut [u32], x: i32, y: i32, v: u32, sign: i32) {
    if let Some(i) = grid::index(x, y) {
        occupancy[i] = if sign > 0 { occupancy[i].wrapping_add(v) } else { occupancy[i].wrapping_sub(v) };
    }
}

/// Add (`sign` 1, `0x1002c7a0`) or remove (`sign` -1, `0x1002c970`) a reservation.
pub(crate) fn occupy(occupancy: &mut [u32], r: Reservation, sign: i32) {
    let (x, y) = (r.x as i32, r.y as i32);
    add(occupancy, x, y, WHOLE_CELL, sign);
    if r.dir == STOP {
        return;
    }
    let (dx, dy) = DIRS[r.dir as usize];
    add(occupancy, x - dx, y - dy, WHOLE_CELL, sign);
    if r.dir & 1 != 0 {
        let k = (r.dir >> 1) as usize;
        add(occupancy, x, y - dy, SIDE_X_OCCUPANCY[k], sign);
        add(occupancy, x - dx, y, SIDE_Y_OCCUPANCY[k], sign);
    }
}

/// Mark a reservation as an obstacle for the path finder (`0x1002cb40`).
pub(crate) fn mark(marks: &mut [u8], r: Reservation) {
    let mut set = |x: i32, y: i32, v: u8, or: bool| {
        if let Some(i) = grid::index(x, y) {
            marks[i] = if or { marks[i] | v } else { v };
        }
    };
    let (x, y) = (r.x as i32, r.y as i32);
    set(x, y, 0xff, false);
    if r.dir == STOP {
        return;
    }
    let (dx, dy) = DIRS[r.dir as usize];
    set(x - dx, y - dy, 0xff, false);
    if r.dir & 1 != 0 {
        // the game subtracts 0x10 from the quarter bit, which also sets the high nibble
        let k = (r.dir >> 1) as usize;
        set(x, y - dy, (SIDE_X_QUARTERS[k] as u8).wrapping_sub(0x10), true);
        set(x - dx, y, (SIDE_Y_QUARTERS[k] as u8).wrapping_sub(0x10), true);
    }
}

/// Move a reservation (`0x1002c740`).
fn reserve(occupancy: &mut [u32], unit: &mut Unit, r: Reservation) {
    if unit.reservation != r {
        occupy(occupancy, unit.reservation, -1);
        unit.reservation = r;
        occupy(occupancy, r, 1);
    }
}

/// Whether a vehicle may drive from `(x, y)` into the neighbour `dir` (`0x1002cf40`).
fn can_enter(rules: &Rules, occupancy: &[u32], unit: &Unit, x: i32, y: i32, dir: u8) -> bool {
    let (dx, dy) = DIRS[dir as usize];
    let (nx, ny) = (x + dx, y + dy);
    let occupied = |x: i32, y: i32| grid::index(x, y).map_or(u32::MAX, |i| occupancy[i]);
    if occupied(nx, ny) & 0xffff != 0 {
        return false;
    }
    let f = rules.grid.at(nx, ny);
    if f & QUARTERS != 0 || f & NO_VEHICLES != 0 {
        return false;
    }
    // bits 20-23: mines known to the player; none are known yet
    let known_mines = 0u32;
    if !rules.types[unit.kind].ignore_mines && known_mines & (f >> 20) & 0xf != 0 {
        return false;
    }
    if dir & 1 != 0 {
        let k = (dir >> 1) as usize;
        if rules.grid.at(nx, y) & SIDE_X_QUARTERS[k] & QUARTERS != 0
            || rules.grid.at(x, ny) & SIDE_Y_QUARTERS[k] & QUARTERS != 0
        {
            return false;
        }
        if occupied(nx, y) & SIDE_X_OCCUPIED[k] != 0 || occupied(x, ny) & SIDE_Y_OCCUPIED[k] != 0 {
            return false;
        }
    }
    true
}

/// One heading step towards `desired` every `TurnDelay` ticks (`0x1002ce60`); the turret turns
/// along while it has no target (`0x10010a60`).
fn turn(unit: &mut Unit, rules: &Rules) {
    let diff = unit.desired.wrapping_sub(unit.facing) & 31;
    if diff == 0 {
        unit.turn_wait = unit.turn_wait.saturating_sub(1);
        return;
    }
    if unit.turn_wait > 0 {
        unit.turn_wait -= 1;
        return;
    }
    let step: u8 = if diff <= 16 { 1 } else { 31 };
    unit.facing = (unit.facing + step) & 31;
    let delay = rules.types[unit.kind].turn_delay(unit.crew);
    if (unit.turn_wait as i32) < delay {
        unit.turn_wait = delay as u8;
    }
    unit.turret = (unit.turret + step) & 31;
}

/// The turret's own turn towards its target, or the hull heading without one (`0x10010b20`).
pub(crate) fn turn_turret(unit: &mut Unit, rules: &Rules) {
    if unit.turret_wait != 0 {
        unit.turret_wait -= 1;
        return;
    }
    let diff = unit.facing.wrapping_sub(unit.turret) & 31;
    if diff == 0 {
        return;
    }
    unit.turret = (unit.turret + if diff <= 16 { 1 } else { 31 }) & 31;
    let delay = rules.types[unit.kind].gun_turn_delay;
    if (unit.turret_wait as i32) < delay {
        unit.turret_wait = delay as u8;
    }
}

/// Distance as the game approximates it: the larger axis plus half the smaller.
fn approx_len(dx: i32, dy: i32) -> i32 {
    let (ax, ay) = (dx.abs(), dy.abs());
    if ay < ax { ay / 2 + ax } else { ax / 2 + ay }
}

/// Last point of the curve the unit is on.
fn curve_end(rules: &Rules, c: CurveState) -> Option<(i16, i16)> {
    rules.curve(c.next, c.previous).last().copied()
}

/// Drive for one tick (`0x1002dee0`).
pub(crate) fn drive(unit: &mut Unit, rules: &Rules, occupancy: &mut [u32], rng: &mut u32, looks: &mut bool) -> Status {
    unit.moving = true;
    loop {
        let dx = unit.aim.0 as i32 - unit.sub.0 as i32;
        let dy = unit.aim.1 as i32 - unit.sub.1 as i32;
        let dist = approx_len(dx, dy);
        if dist > REACHED {
            let h = heading(dx, dy);
            if unit.desired == h {
                unit.reverse = false;
            } else {
                unit.desired = if unit.reverse { h.wrapping_sub(16) & 31 } else { h };
            }
            turn(unit, rules);
            if unit.facing != unit.desired {
                return Status::Turning;
            }
            let step = rules.types[unit.kind].speed(unit.crew).min(dist);
            let (cx, cy) = (unit.cell.0 as i32, unit.cell.1 as i32);
            let before = ((unit.sub.0 >> 13) as i32 + cx, (unit.sub.1 >> 13) as i32 + cy);
            unit.sub.0 = unit.sub.0.wrapping_add((step * dx / dist) as i16);
            unit.sub.1 = unit.sub.1.wrapping_add((step * dy / dist) as i16);
            let after = ((unit.sub.0 >> 13) as i32 + cx, (unit.sub.1 >> 13) as i32 + cy);
            if before.0 != after.0 || before.1 != after.1 {
                // crossing into another cell's area looks around again one time in two (`0x1002e415`)
                *rng = lcg(*rng);
                *looks = *rng & 0x10000 != 0;
            }
            return Status::Moved;
        }
        turn(unit, rules);
        unit.sub = unit.aim;
        unit.curve.point = unit.curve.point.wrapping_add(1);
        while rules.curve(unit.curve.next, unit.curve.previous).len() <= unit.curve.point as usize {
            // (crossing a cell also crushes objects and triggers mines there: not done yet)
            if let Some(status) = next_cell(unit, rules, occupancy, rng) {
                return status;
            }
        }
        let p = rules.curve(unit.curve.next, unit.curve.previous)[unit.curve.point as usize];
        unit.aim = p;
    }
}

/// At the end of a curve: enter the next cell of the path, or stop in this one.
fn next_cell(unit: &mut Unit, rules: &Rules, occupancy: &mut [u32], rng: &mut u32) -> Option<Status> {
    let (x, y) = (unit.cell.0 as i32, unit.cell.1 as i32);
    let entering = unit.path.next().filter(|&dir| can_enter(rules, occupancy, unit, x, y, dir));
    let Some(dir) = entering else {
        if unit.curve.next == STOP {
            return Some(arrived(unit));
        }
        reserve(occupancy, unit, Reservation { x: x as i16, y: y as i16, dir: STOP });
        unit.curve = CurveState { previous: unit.curve.next, next: STOP, point: 0 };
        return None;
    };
    unit.path.index = (unit.path.index + 1) % 64;
    let (dx, dy) = DIRS[dir as usize];
    unit.cell = ((x + dx) as i16, (y + dy) as i16);
    reserve(occupancy, unit, Reservation { x: unit.cell.0, y: unit.cell.1, dir });
    unit.sub.0 = unit.sub.0.wrapping_add((dx * -0x2000) as i16);
    unit.sub.1 = unit.sub.1.wrapping_add((dy * -0x2000) as i16);
    unit.curve = CurveState { previous: unit.curve.next, next: dir, point: 0 };
    if rules.types[unit.kind].can_move_backward
        && let Some(end) = curve_end(rules, unit.curve)
    {
        // drive backwards when the end of the curve is far behind (`0x1002e145`)
        let h = heading(end.0 as i32 - unit.sub.0 as i32, end.1 as i32 - unit.sub.1 as i32);
        let mut off = h.wrapping_sub(unit.facing) as i32 & 31;
        if off > 16 {
            off = 32 - off;
        }
        *rng = lcg(*rng);
        let r = (*rng as i32) >> 16;
        unit.reverse = if unit.path.index < 5 { (r & 1) + 14 < off } else { off > (r & 7) + 8 };
    }
    None
}

/// Stopped at the centre of a cell with no way on (`0x1002e055`).
fn arrived(unit: &mut Unit) -> Status {
    let Some(goal) = unit.goal else {
        unit.moving = false;
        return Status::Arrived;
    };
    if unit.cell == goal {
        unit.moving = false;
        return Status::Arrived;
    }
    match unit.path.status {
        PathStatus::Wanted => Status::Waiting,
        PathStatus::Failed => {
            unit.moving = false;
            Status::GaveUp
        }
        _ => {
            unit.path.status = PathStatus::Wanted;
            Status::Waiting
        }
    }
}
