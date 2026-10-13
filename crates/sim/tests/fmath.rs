use openstrike_sim::fmath::{atan2, heading, sqrt};

#[test]
fn square_root_matches_the_game() {
    // the game's routine starts at 1 and halves towards 0 for 0
    assert_eq!(sqrt(0.0), 1.0 / 32.0);
    assert_eq!(sqrt(1.0), 1.0);
    for v in [2.0, 4.0, 100.0, 12345.0, 65536.0, 130_050.0] {
        assert!((sqrt(v) - f64::sqrt(v)).abs() < 1e-6, "sqrt({v})");
    }
}

#[test]
fn arctangent_is_close_to_the_library_one() {
    for (y, x) in [(1.0, 1.0), (0.3, 2.0), (-5.0, 1.0), (-1.0, -1.0), (7.0, -0.5), (0.0, -3.0)] {
        assert!((atan2(y, x) - f64::atan2(y, x)).abs() < 1e-6, "atan2({y}, {x})");
    }
    assert_eq!(atan2(0.0, 0.0), 0.0);
}

#[test]
fn headings_are_clockwise_from_east() {
    assert_eq!(heading(1, 0), 0);
    assert_eq!(heading(1, 1), 4);
    assert_eq!(heading(0, 1), 8);
    assert_eq!(heading(-1, 0), 16);
    assert_eq!(heading(0, -1), 24);
    assert_eq!(heading(100, 19), 1);
    assert_eq!(heading(100, -19), 31);
}
