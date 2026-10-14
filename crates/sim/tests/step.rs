use openstrike_sim::grid::{DIRS, STOP};
use openstrike_sim::{Command, Grid, Order, PathStatus, Rules, Unit, UnitId, UnitType, World, group_targets, step};

/// Curves shaped like the game's: one point on the edge the unit enters by, the stop point at
/// (15, 15) px, nothing for "stopped to stopped".
fn curves() -> Vec<Vec<(i16, i16)>> {
    let mut out = Vec::new();
    for next in 0..9u8 {
        for _previous in 0..9u8 {
            out.push(match next {
                STOP => vec![(0xf00, 0xf00)],
                d => {
                    let (dx, dy) = DIRS[d as usize];
                    vec![((0xf00 - dx * 0xf00) as i16, (0xf00 - dy * 0xf00) as i16)]
                }
            });
        }
    }
    out[STOP as usize * 9 + STOP as usize].clear();
    out
}

fn tank() -> UnitType {
    let mut t = UnitType::new("t-26", (1.4, 1.5), (5, 3), 1);
    t.can_move_backward = false;
    t
}

fn rules(grid: Grid) -> Rules {
    Rules { grid, curves: curves(), types: vec![tank()], circles: Vec::new(), sides: Vec::new() }
}

fn open() -> Rules {
    rules(Grid::open(64, 64))
}

fn run(mut w: World, rules: &Rules, ticks: usize, first: &[Command]) -> World {
    w = step(&w, rules, first);
    for _ in 1..ticks {
        w = step(&w, rules, &[]);
    }
    w
}

fn one_tank(cell: (i16, i16), facing: u8) -> World {
    World::new(vec![Unit::new(UnitId(1), 0, cell, facing)], 1)
}

fn move_to(x: i16, y: i16) -> Command {
    Command::Move { units: vec![UnitId(1)], to: (x, y) }
}

#[test]
fn speeds_and_delays_come_from_the_description() {
    let t = tank();
    assert_eq!((t.move_min, t.move_range), (358, 26));
    assert_eq!(t.speed(0x8000), 384);
    assert_eq!(t.turn_delay(0x8000), 3);
}

#[test]
fn idle_units_stay_put() {
    let w = run(one_tank((10, 10), 0), &open(), 10, &[]);
    assert_eq!(w.units[0].cell, (10, 10));
    assert_eq!(w.units[0].sub, (0xf00, 0xf00));
    assert_eq!(w.tick, 10);
}

#[test]
fn units_reach_their_goal_cell_and_stop_in_its_middle() {
    let w = run(one_tank((10, 10), 0), &open(), 400, &[move_to(20, 14)]);
    let u = &w.units[0];
    assert_eq!(u.cell, (20, 14));
    assert_eq!(u.sub, (0xf00, 0xf00));
    assert_eq!(u.order, Order::Idle);
    assert_eq!(u.goal, None);
}

#[test]
fn a_straight_run_takes_cell_length_over_speed() {
    // 10 cells east at 384/256 px per tick: 8192 * 10 / 384 = 213 ticks, less the snapping to
    // each curve point within 0xfa
    let mut w = step(&one_tank((10, 10), 0), &open(), &[move_to(20, 10)]);
    let mut ticks = 1;
    while w.units[0].order != Order::Idle && ticks < 1000 {
        w = step(&w, &open(), &[]);
        ticks += 1;
    }
    assert_eq!(w.units[0].cell, (20, 10));
    assert!((205..=215).contains(&ticks), "took {ticks} ticks");
}

#[test]
fn units_turn_in_place_before_driving() {
    // facing west, ordered east: 16 heading steps, one every 4 ticks, before any move
    let mut w = step(&one_tank((10, 10), 16), &open(), &[move_to(14, 10)]);
    let start = w.units[0].position();
    for _ in 0..80 {
        w = step(&w, &open(), &[]);
        if w.units[0].facing == 0 {
            break;
        }
        assert_eq!(w.units[0].position(), start, "moved while turning");
    }
    assert_eq!(w.units[0].facing, 0);
}

#[test]
fn units_go_around_a_wall() {
    // a wall across x = 15, y = 5..30, the goal behind it
    let wall: Vec<(i32, i32)> = (5..30).map(|y| (15, y)).collect();
    let r = rules(Grid::open(64, 64).with_blocked(wall.clone(), 0xf));
    let mut w = step(&one_tank((10, 15), 0), &r, &[move_to(20, 15)]);
    for _ in 0..3000 {
        w = step(&w, &r, &[]);
        let c = w.units[0].cell;
        assert!(!wall.contains(&(c.0 as i32, c.1 as i32)), "drove into the wall at {c:?}");
        if w.units[0].order == Order::Idle {
            break;
        }
    }
    assert_eq!(w.units[0].cell, (20, 15));
}

#[test]
fn an_unreachable_goal_is_given_up() {
    // the goal is walled in on every side
    let mut walls = Vec::new();
    for i in 25..=35 {
        walls.extend([(i, 25), (i, 35), (25, i), (35, i)]);
    }
    let r = rules(Grid::open(64, 64).with_blocked(walls, 0xf));
    let w = run(one_tank((10, 10), 0), &r, 3000, &[move_to(30, 30)]);
    assert_eq!(w.units[0].order, Order::Idle);
    assert_ne!(w.units[0].cell, (30, 30));
}

#[test]
fn two_units_never_share_a_cell() {
    let r = open();
    let units = vec![Unit::new(UnitId(1), 0, (10, 10), 0), Unit::new(UnitId(2), 0, (20, 10), 16)];
    let mut w = step(&World::new(units, 7), &r, &[move_to(20, 10), Command::Move { units: vec![UnitId(2)], to: (10, 10) }]);
    for _ in 0..1500 {
        w = step(&w, &r, &[]);
        assert_ne!(w.units[0].cell, w.units[1].cell);
    }
}

#[test]
fn occupancy_follows_the_units() {
    let r = open();
    let w = run(one_tank((10, 10), 0), &r, 300, &[move_to(16, 13)]);
    let held: Vec<usize> = (0..w.occupancy.len()).filter(|&i| w.occupancy[i] != 0).collect();
    assert_eq!(held, vec![13 * 256 + 16]);
    assert_eq!(w.occupancy[13 * 256 + 16], 0x1111_1111);
}

#[test]
fn the_same_commands_give_the_same_world() {
    let r = open();
    let units = vec![Unit::new(UnitId(1), 0, (10, 10), 0), Unit::new(UnitId(2), 0, (12, 10), 0)];
    let cmd = [Command::Move { units: vec![UnitId(1), UnitId(2)], to: (30, 30) }];
    let a = run(World::new(units.clone(), 3), &r, 400, &cmd);
    let b = run(World::new(units, 3), &r, 400, &cmd);
    assert_eq!(a, b);
}

#[test]
fn a_path_is_planned_the_tick_after_the_order() {
    let r = open();
    let w = step(&one_tank((10, 10), 0), &r, &[move_to(13, 10)]);
    // the unit stood still, asked for a path, and the scheduler planned it in the same tick
    assert_eq!(w.units[0].path.status, PathStatus::Ready);
    assert_eq!(w.units[0].path.remaining(), vec![0, 0, 0]);
}

#[test]
fn a_tight_group_keeps_its_shape() {
    // 3 tanks in a row, ordered 20 cells down: each keeps its offset from the group's centre
    let r = open();
    let units = vec![Unit::new(UnitId(1), 0, (10, 10), 0), Unit::new(UnitId(2), 0, (12, 10), 0), Unit::new(UnitId(3), 0, (14, 10), 0)];
    let w = World::new(units, 0);
    let members: Vec<&Unit> = w.units.iter().collect();
    assert_eq!(group_targets(&r, &w.occupancy, &members, (30, 30)), vec![(28, 30), (30, 30), (32, 30)]);
}

#[test]
fn a_spread_group_gathers_on_every_other_cell() {
    // 30 cells apart: odd rows and columns of the slot grid are left empty, and the target's own
    // slot (15, 15) is odd, so the spiral starts on the ring around it
    let r = open();
    let units = vec![Unit::new(UnitId(1), 0, (5, 5), 0), Unit::new(UnitId(2), 0, (40, 40), 0), Unit::new(UnitId(3), 0, (5, 40), 0)];
    let w = World::new(units, 0);
    let members: Vec<&Unit> = w.units.iter().collect();
    assert_eq!(group_targets(&r, &w.occupancy, &members, (30, 30)), vec![(29, 29), (31, 29), (29, 31)]);
}

#[test]
fn a_place_held_by_another_unit_is_moved_along_the_spiral() {
    // the target cell holds a unit: the ordered one goes to the first free cell of the spiral
    let r = open();
    let units = vec![Unit::new(UnitId(1), 0, (10, 10), 0), Unit::new(UnitId(2), 0, (30, 30), 0)];
    let w = World::new(units, 0);
    assert_eq!(group_targets(&r, &w.occupancy, &[&w.units[0]], (30, 30)), vec![(29, 29)]);
}

#[test]
fn halte_stops_short_of_the_goal() {
    let r = open();
    let w = run(one_tank((10, 10), 0), &r, 60, &[move_to(30, 10)]);
    let w = run(w, &r, 200, &[Command::Stop { units: vec![UnitId(1)] }]);
    let u = &w.units[0];
    assert_eq!((u.order, u.goal), (Order::Idle, None));
    // it ends in the middle of a cell on the way, well before the goal
    assert_eq!(u.sub, (0xf00, 0xf00));
    assert!(u.cell.0 > 10 && u.cell.0 < 30, "{:?}", u.cell);
    assert!(u.turret_wait <= 2);
}

#[test]
fn hold_orders_set_the_unit_flags() {
    let w = step(&one_tank((10, 10), 0), &open(), &[Command::HoldPosition { units: vec![UnitId(1)], on: true }]);
    assert!(w.units[0].hold_position && !w.units[0].hold_fire);
    let w = step(&w, &open(), &[Command::HoldFire { units: vec![UnitId(1)], on: true }, Command::HoldPosition { units: vec![UnitId(1)], on: false }]);
    assert!(!w.units[0].hold_position && w.units[0].hold_fire);
}
