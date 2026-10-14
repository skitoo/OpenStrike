use openstrike::mission::{friendly, hostile};
use openstrike_formats::map::MapPlayer;

fn players(sides: &[u8]) -> Vec<MapPlayer> {
    sides.iter().map(|&side| MapPlayer { name: String::new(), side, extra: [0; 4] }).collect()
}

#[test]
fn single_player_sides() {
    // Player, Enemy, Ally, Neutral, as in most maps
    let p = players(&[0, 1, 2, 3]);
    assert!(hostile(&p, 0, 1) && hostile(&p, 1, 0) && hostile(&p, 1, 2));
    assert!(!hostile(&p, 0, 2) && !hostile(&p, 0, 0));
    // side 3 is in no table: neither friend nor foe
    assert!(!hostile(&p, 0, 3) && !hostile(&p, 3, 1));
    // some maps put the "ally" on the enemy's side
    let p = players(&[0, 1, 1, 3]);
    assert!(hostile(&p, 0, 2));
    // players the map does not list are on side 0
    assert!(hostile(&p, 5, 1) && !hostile(&p, 5, 0));
}

#[test]
fn friends_include_the_player_itself_and_its_allies() {
    let p = players(&[0, 1, 2, 3]);
    assert!(friendly(&p, 0, 0) && friendly(&p, 0, 2) && friendly(&p, 1, 1));
    assert!(!friendly(&p, 0, 1) && !friendly(&p, 0, 3));
}
