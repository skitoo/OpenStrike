use openstrike::status::{bars, group_at, Line, Owner, Status, AMMO, BACKGROUND, CREW, CREW_BAR, DAMAGED, HEALTHY, HOSTILE, OTHER, WRECKED};

fn pz4g(owner: Owner) -> Status {
    Status { owner, hp: 450, max_hp: 450, damaged_below: 300, crew: 0x8000, veteran: false, ammo: [122, 2700], max_ammo: [122, 2700] }
}

fn line(x: i32, y: i32, len: i32, colour: usize) -> Line {
    Line { x, y, len, colour }
}

#[test]
fn a_full_tank_of_the_local_player() {
    // Selector 15: 30 pixels wide, from 11 pixels under the ground point
    assert_eq!(
        bars((100, 50), 15, &pz4g(Owner::Local)),
        vec![
            line(85, 61, 30, AMMO),
            line(85, 62, 30, AMMO),
            line(85, 63, 30, CREW_BAR),
            line(85, 63, 30, CREW),
            line(85, 64, 30, HEALTHY),
            line(85, 65, 30, HEALTHY),
        ]
    );
}

#[test]
fn hostile_units_show_crew_and_hit_points_only() {
    let mut s = pz4g(Owner::Hostile);
    s.hp = 225;
    s.crew = 0x4000;
    assert_eq!(
        bars((100, 50), 15, &s),
        vec![
            line(100, 61, 15, BACKGROUND),
            line(85, 61, 15, CREW_BAR),
            line(85, 61, 15, CREW),
            line(100, 62, 15, BACKGROUND),
            line(85, 62, 15, HOSTILE),
            line(100, 63, 15, BACKGROUND),
            line(85, 63, 15, HOSTILE),
        ]
    );
}

#[test]
fn the_hit_point_colour_follows_the_damage() {
    let colour = |hp: i32| {
        let mut s = pz4g(Owner::Local);
        s.hp = hp;
        bars((0, 0), 15, &s).last().unwrap().colour
    };
    assert_eq!(colour(300), HEALTHY);
    assert_eq!(colour(299), DAMAGED); // under MoveDamageHP
    assert_eq!(colour(112), DAMAGED);
    assert_eq!(colour(111), WRECKED); // under a quarter (450 >> 2)
}

#[test]
fn veterans_have_a_longer_crew_bar() {
    let mut s = pz4g(Owner::Hostile);
    s.crew = 0x4000;
    s.veteran = true;
    let crew: Vec<Line> = bars((0, 0), 15, &s).into_iter().filter(|l| l.y == 11).collect();
    // 0x4000 + 0x2000 shown: 22 of 30 pixels, the crew's own 15 over it
    assert_eq!(crew, vec![line(7, 11, 8, BACKGROUND), line(-15, 11, 22, CREW_BAR), line(-15, 11, 15, CREW)]);
}

#[test]
fn a_type_without_selector_draws_nothing_visible() {
    assert!(bars((0, 0), 0, &pz4g(Owner::Local)).is_empty());
}

#[test]
fn units_of_players_not_hostile_have_their_own_colour() {
    assert_eq!(bars((0, 0), 15, &pz4g(Owner::Other)).last().unwrap().colour, OTHER);
}

#[test]
fn the_group_number_sits_over_the_right_end() {
    assert_eq!(group_at((100, 50), 15), (111, 56));
}
