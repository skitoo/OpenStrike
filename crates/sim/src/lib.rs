//! Deterministic game simulation.
//!
//! The whole game state is a value ([`World`]) and advances only through [`step`], at a fixed rate,
//! from the commands issued during the tick. All arithmetic is integer, so the same commands give
//! the same world on every machine: that is what replays and lockstep multiplayer will rely on.

pub mod geom;
pub mod path;
pub mod terrain;

pub use geom::{Direction, Vec2};
pub use terrain::Terrain;

/// Simulation ticks per second.
pub const TICKS_PER_SECOND: u32 = 20;

/// Sub-units per map pixel.
pub const SUBPIXEL: i32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    Idle,
    /// Remaining waypoints, in sub-units; the last one is the destination.
    Move(Vec<Vec2>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub id: UnitId,
    /// Name of the unit type, as in the game's description files (`t-26`).
    pub kind: String,
    /// Position in sub-units.
    pub pos: Vec2,
    pub facing: Direction,
    /// Speed in sub-units per tick.
    pub speed: i32,
    pub order: Order,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Move { unit: UnitId, to: Vec2 },
    /// Move several units to the same spot: each gets its own place around it (see [`formation`]).
    MoveGroup { units: Vec<UnitId>, to: Vec2 },
}

/// Spacing between units of a group, in sub-units (three terrain tiles, about a tank length).
pub const FORMATION_SPACING: i32 = 3 * terrain::TILE * SUBPIXEL;

/// Places for `count` units around `to`: the centre, then rings outwards, in a fixed order.
pub fn formation(to: Vec2, count: usize) -> Vec<Vec2> {
    let mut places = Vec::with_capacity(count);
    let mut ring = 0i32;
    while places.len() < count {
        if ring == 0 {
            places.push(to);
        } else {
            // walk the square ring of radius `ring`, starting at its top-left corner
            for i in 0..8 * ring {
                let (side, k) = (i / (2 * ring), i % (2 * ring));
                let (dx, dy) = match side {
                    0 => (-ring + k, -ring),
                    1 => (ring, -ring + k),
                    2 => (ring - k, ring),
                    _ => (-ring, ring - k),
                };
                places.push(to + Vec2::new(dx * FORMATION_SPACING, dy * FORMATION_SPACING));
                if places.len() == count {
                    break;
                }
            }
        }
        ring += 1;
    }
    places
}

/// The destination each command gives to each unit, the last command winning.
fn destinations(commands: &[Command]) -> Vec<(UnitId, Vec2)> {
    let mut out = Vec::new();
    for c in commands {
        match c {
            Command::Move { unit, to } => out.push((*unit, *to)),
            Command::MoveGroup { units, to } => {
                let mut sorted = units.clone();
                sorted.sort();
                sorted.dedup();
                out.extend(sorted.iter().copied().zip(formation(*to, sorted.len())));
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    pub tick: u64,
    /// Map size in sub-units.
    pub size: Vec2,
    pub units: Vec<Unit>,
}

impl World {
    pub fn new(size: Vec2, units: Vec<Unit>) -> Self {
        Self { tick: 0, size, units }
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.iter().find(|u| u.id == id)
    }
}

/// Advance the world by one tick.
pub fn step(world: &World, terrain: &Terrain, commands: &[Command]) -> World {
    let destinations = destinations(commands);
    let units = world
        .units
        .iter()
        .map(|unit| {
            let order = destinations
                .iter()
                .rfind(|(id, _)| *id == unit.id)
                .map(|(_, to)| plan(terrain, unit.pos, clamp(*to, world.size)))
                .unwrap_or_else(|| unit.order.clone());
            advance(Unit { order, ..unit.clone() })
        })
        .collect();
    World { tick: world.tick + 1, size: world.size, units }
}

fn clamp(p: Vec2, size: Vec2) -> Vec2 {
    Vec2::new(p.x.clamp(0, size.x - 1), p.y.clamp(0, size.y - 1))
}

/// Waypoints from `from` to `to` around blocked tiles. A blocked destination is replaced by the
/// closest passable tile; an unreachable one gives `Idle`.
pub fn plan(terrain: &Terrain, from: Vec2, to: Vec2) -> Order {
    let start = Terrain::tile_of(from);
    let wanted = Terrain::tile_of(to);
    let Some(goal) = path::nearest_passable(terrain, wanted) else { return Order::Idle };
    let Some(tiles) = path::find(terrain, start, goal) else { return Order::Idle };
    let mut waypoints: Vec<Vec2> = path::smooth(terrain, start, &tiles).into_iter().map(|(x, y)| Terrain::centre(x, y)).collect();
    let destination = if goal == wanted { to } else { Terrain::centre(goal.0, goal.1) };
    match waypoints.last_mut() {
        Some(last) => *last = destination,
        None => waypoints.push(destination),
    }
    Order::Move(waypoints)
}

fn advance(mut unit: Unit) -> Unit {
    let mut budget = unit.speed as i64;
    while let Order::Move(waypoints) = &mut unit.order {
        let Some(&target) = waypoints.first() else {
            unit.order = Order::Idle;
            break;
        };
        let delta = target - unit.pos;
        let distance = delta.length();
        if distance > 0 {
            unit.facing = Direction::towards(delta);
        }
        if distance <= budget {
            unit.pos = target;
            budget -= distance;
            waypoints.remove(0);
            if waypoints.is_empty() {
                unit.order = Order::Idle;
            }
            if budget == 0 {
                break;
            }
        } else {
            unit.pos = unit.pos
                + Vec2::new((delta.x as i64 * budget / distance) as i32, (delta.y as i64 * budget / distance) as i32);
            break;
        }
    }
    unit
}
