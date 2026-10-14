use openstrike::scene::UnitView;
use openstrike::selection::{outline, select, unit_at, units_in};
use openstrike_sim::UnitId;

fn view(id: u32, x: i32, y: i32) -> UnitView {
    UnitView { id: UnitId(id), kind: 0, player: 0, screen: (x, y), frame: 0, turret_frame: 0 }
}

#[test]
fn clicks_pick_the_front_most_unit() {
    let units = [view(1, 100, 100), view(2, 105, 110)];
    assert_eq!(unit_at(&units, (102.0, 100.0)), Some(UnitId(2)));
    assert_eq!(unit_at(&units, (300.0, 300.0)), None);
}

#[test]
fn boxes_pick_units_by_ground_position() {
    let units = [view(1, 100, 100), view(2, 200, 100), view(3, 400, 400)];
    let mut picked = units_in(&units, (250.0, 50.0), (50.0, 150.0));
    picked.sort();
    assert_eq!(picked, vec![UnitId(1), UnitId(2)]);
}

#[test]
fn shift_adds_and_a_click_on_nothing_clears() {
    let units = [view(1, 100, 100), view(2, 200, 100)];
    let one = select(&[], &units, (100.0, 100.0), (100.0, 100.0), false);
    assert_eq!(one, vec![UnitId(1)]);
    let both = select(&one, &units, (200.0, 100.0), (201.0, 100.0), true);
    assert_eq!(both, vec![UnitId(1), UnitId(2)]);
    assert!(select(&both, &units, (600.0, 600.0), (600.0, 600.0), false).is_empty());
}

#[test]
fn the_rectangle_includes_both_corners_from_the_press_on() {
    assert_eq!(outline((10, 20), (10, 20)), (10, 20, 1, 1));
    assert_eq!(outline((30, 5), (10, 20)), (10, 5, 21, 16));
}
