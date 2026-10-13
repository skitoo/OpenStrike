use openstrike_sim::terrain::TILE;
use openstrike_sim::{Command, Direction, Order, SUBPIXEL, Terrain, Unit, UnitId, Vec2, World, step};

const T: i32 = TILE * SUBPIXEL;

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
    World::new(Vec2::new(4096 * SUBPIXEL, 4096 * SUBPIXEL), vec![tank(T / 2, T / 2)])
}

fn open() -> Terrain {
    Terrain::open(256, 256)
}

fn run(mut w: World, terrain: &Terrain, ticks: usize, first: &[Command]) -> World {
    w = step(&w, terrain, first);
    for _ in 1..ticks {
        w = step(&w, terrain, &[]);
    }
    w
}

fn move_to(x: i32, y: i32) -> Command {
    Command::Move { unit: UnitId(1), to: Vec2::new(x, y) }
}

#[test]
fn idle_units_stay_put() {
    let w = run(world(), &open(), 10, &[]);
    assert_eq!(w.units[0].pos, Vec2::new(T / 2, T / 2));
    assert_eq!(w.tick, 10);
}

#[test]
fn units_reach_their_target_and_stop() {
    let to = Vec2::new(100 * SUBPIXEL, 50 * SUBPIXEL);
    let w = run(world(), &open(), 100, &[move_to(to.x, to.y)]);
    assert_eq!(w.units[0].pos, to);
    assert_eq!(w.units[0].order, Order::Idle);
}

#[test]
fn moving_units_face_their_target() {
    let w = step(&world(), &open(), &[move_to(T / 2, 100 * SUBPIXEL)]);
    assert_eq!(w.units[0].facing, Direction(8));
}

#[test]
fn units_go_around_a_wall() {
    // a wall across x = 3 tiles, from y = 0 to 9, with a gap below
    let terrain = open().with_blocked((0..10).map(|y| (3, y)));
    let target = Vec2::new(6 * T + T / 2, T / 2);
    let mut w = step(&world(), &terrain, &[move_to(target.x, target.y)]);
    for _ in 0..400 {
        let (tx, ty) = Terrain::tile_of(w.units[0].pos);
        assert!(terrain.passable(tx, ty), "drove through the wall at {tx},{ty}");
        w = step(&w, &terrain, &[]);
    }
    assert_eq!(w.units[0].pos, target);
}

#[test]
fn a_blocked_destination_stops_at_the_closest_free_tile() {
    let terrain = open().with_blocked((5..10).flat_map(|x| (5..10).map(move |y| (x, y))));
    let w = run(world(), &terrain, 300, &[move_to(7 * T, 7 * T)]);
    let (tx, ty) = Terrain::tile_of(w.units[0].pos);
    assert!(terrain.passable(tx, ty));
    assert!((4..=10).contains(&tx) && (4..=10).contains(&ty), "{tx},{ty}");
}

#[test]
fn an_unreachable_destination_is_ignored() {
    // the tank is boxed in
    let terrain = open().with_blocked([(1, 0), (0, 1), (1, 1)]);
    let w = step(&world(), &terrain, &[move_to(50 * T, 50 * T)]);
    assert_eq!(w.units[0].order, Order::Idle);
}

#[test]
fn move_targets_are_clamped_to_the_map() {
    let w = run(world(), &open(), 20, &[move_to(-5000, -5000)]);
    assert_eq!(w.units[0].pos, Vec2::new(0, 0));
}

#[test]
fn same_commands_give_the_same_world() {
    let terrain = open().with_blocked((0..20).map(|y| (5, y)));
    let cmds = [move_to(12345, 6789)];
    assert_eq!(run(world(), &terrain, 37, &cmds), run(world(), &terrain, 37, &cmds));
}

#[test]
fn step_does_not_touch_its_input() {
    let before = world();
    let _ = step(&before, &open(), &[move_to(500, 500)]);
    assert_eq!(before, world());
}
