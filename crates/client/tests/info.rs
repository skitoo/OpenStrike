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
