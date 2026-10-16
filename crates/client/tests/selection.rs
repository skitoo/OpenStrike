use openstrike::scene::UnitView;
use openstrike::selection::{Groups, assign, centre, outline, recall, select, unit_at, units_in};
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

fn owned(id: u32, player: u8) -> UnitView {
    let mut v = view(id, 0, 0);
    v.player = player;
    v
}

#[test]
fn ctrl_digit_makes_the_selection_a_group() {
    let ids = [UnitId(1), UnitId(2), UnitId(3)];
    let mut groups = Groups::new();
    assign(&mut groups, &ids, &[UnitId(1), UnitId(2)], 2, false);
    assert_eq!((groups.get(&UnitId(1)), groups.get(&UnitId(2)), groups.get(&UnitId(3))), (Some(&2), Some(&2), None));
    // a new group 2 of unit 3 alone: 1 and 2 leave it, unless Shift keeps them
    let mut kept = groups.clone();
    assign(&mut groups, &ids, &[UnitId(3)], 2, false);
    assert_eq!((groups.get(&UnitId(1)), groups.get(&UnitId(3))), (None, Some(&2)));
    assign(&mut kept, &ids, &[UnitId(3)], 2, true);
    assert_eq!((kept.get(&UnitId(1)), kept.get(&UnitId(3))), (Some(&2), Some(&2)));
}

#[test]
fn a_digit_recalls_its_group() {
    let units = [owned(1, 0), owned(2, 0), owned(3, 0), owned(4, 1)];
    let groups: Groups = [(UnitId(1), 1), (UnitId(2), 1), (UnitId(4), 1)].into_iter().collect();
    // unit 4 belongs to another player: dropped since the local player's units are there
    assert_eq!(recall(&units, &[UnitId(3)], &groups, 1, false, 0), [UnitId(1), UnitId(2)]);
    // Shift adds the group to the selection
    assert_eq!(recall(&units, &[UnitId(3)], &groups, 1, true, 0), [UnitId(1), UnitId(2), UnitId(3)]);
    assert!(recall(&units, &[UnitId(3)], &groups, 5, false, 0).is_empty());
}

#[test]
fn a_second_press_centres_on_the_mean_cell() {
    assert_eq!(centre(&[(10, 20), (11, 20)]), Some((336, 640)));
    assert_eq!(centre(&[]), None);
}
