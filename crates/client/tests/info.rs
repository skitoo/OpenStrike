use std::collections::HashMap;

use openstrike::info::{expand, layout};

#[test]
fn key_names_replace_their_marks() {
    let keys: HashMap<String, String> = [("00".into(), "A".into()), ("hm".into(), "R".into())].into_iter().collect();
    assert_eq!(expand("AVANCER (~key_00~)", &keys), "AVANCER (A)");
    assert_eq!(expand("TENIR LA POSITION (~key_HM~)", &keys), "TENIR LA POSITION (R)");
    assert_eq!(expand("RIEN (~key_99~)", &keys), "RIEN ()");
    assert_eq!(expand("SANS TOUCHE", &keys), "SANS TOUCHE");
}

#[test]
fn glyphs_advance_by_their_width_and_two() {
    let width = |c: u8| if c == b'I' { 1 } else { 5 };
    assert_eq!(layout("AIA", width), [(0, b'A'), (7, b'I'), (10, b'A')]);
}

use openstrike::info::{gauge, layout_centred, unit_fields, Field};

#[test]
fn a_unit_shows_hit_points_crew_and_its_ammunition_to_friends() {
    let f = unit_fields(450, 450, 0x8000, 0x8000, [61, 2700], [122, 2700], true);
    assert_eq!(f[0], Some(Field { value: "450".into(), percent: 100, under: None }));
    assert_eq!(f[1], Some(Field { value: "1000".into(), percent: 100, under: Some(100) }));
    assert_eq!(f[2], Some(Field { value: "61".into(), percent: 50, under: None }));
    // others do not show their ammunition
    let f = unit_fields(1, 450, 0x4000, 0x6000, [61, 2700], [122, 2700], false);
    assert_eq!(f[0].as_ref().unwrap().percent, 1); // at least 1 %
    assert_eq!(f[1].as_ref().unwrap().value, "500/750"); // a veteran crew: own / shown
    assert_eq!((f[2].clone(), f[3].clone()), (None, None));
}

#[test]
fn gauges_and_values() {
    assert_eq!(gauge(53, 50), 26);
    assert_eq!(gauge(53, 0), 0);
    // "12": two glyphs 4 wide, 12 pixels of advance in all, from -6
    assert_eq!(layout_centred("12", |_| 4), [(-6, b'1'), (0, b'2')]);
}
