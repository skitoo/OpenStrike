use openstrike_sim::{Command, Direction, Order, SUBPIXEL, Unit, UnitId, Vec2, World, step};

fn tank(x: i32, y: i32) -> Unit {
    Unit {
        id: UnitId(1),
        kind: "t-26".into(),
        pos: Vec2::new(x, y),
        facing: Direction(0),
        speed: 2 * SUBPIXEL,
        order: Order::Idle,
    }
}

fn world() -> World {
    World::new(Vec2::new(4096 * SUBPIXEL, 4096 * SUBPIXEL), vec![tank(0, 0)])
}

fn run(mut w: World, ticks: usize, first: &[Command]) -> World {
    w = step(&w, first);
    for _ in 1..ticks {
        w = step(&w, &[]);
    }
    w
}

#[test]
fn idle_units_stay_put() {
    let w = run(world(), 10, &[]);
    assert_eq!(w.units[0].pos, Vec2::new(0, 0));
    assert_eq!(w.tick, 10);
}

#[test]
fn units_reach_their_target_and_stop() {
    let to = Vec2::new(100 * SUBPIXEL, 50 * SUBPIXEL);
    let w = run(world(), 100, &[Command::Move { unit: UnitId(1), to }]);
    assert_eq!(w.units[0].pos, to);
    assert_eq!(w.units[0].order, Order::Idle);
}

#[test]
fn moving_units_face_their_target() {
    let w = step(&world(), &[Command::Move { unit: UnitId(1), to: Vec2::new(0, 100 * SUBPIXEL) }]);
    assert_eq!(w.units[0].facing, Direction(8));
}

#[test]
fn move_targets_are_clamped_to_the_map() {
    let w = step(&world(), &[Command::Move { unit: UnitId(1), to: Vec2::new(-5, -5) }]);
    assert_eq!(w.units[0].order, Order::Idle);
    assert_eq!(w.units[0].pos, Vec2::new(0, 0));
}

#[test]
fn same_commands_give_the_same_world() {
    let cmds = [Command::Move { unit: UnitId(1), to: Vec2::new(12345, 6789) }];
    assert_eq!(run(world(), 37, &cmds), run(world(), 37, &cmds));
}

#[test]
fn step_does_not_touch_its_input() {
    let before = world();
    let _ = step(&before, &[Command::Move { unit: UnitId(1), to: Vec2::new(500, 500) }]);
    assert_eq!(before, world());
}
