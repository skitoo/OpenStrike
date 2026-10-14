use openstrike::scene::{cursor_distance, see_through};

#[test]
fn cursor_distance_counts_screen_y_twice() {
    assert_eq!(cursor_distance((100, 100), (100, 100)), 0);
    assert_eq!(cursor_distance((100, 100), (130, 100)), 30);
    assert_eq!(cursor_distance((100, 100), (100, 130)), 60);
    // larger plus half the smaller, truncated
    assert_eq!(cursor_distance((0, 0), (10, 3)), 13);
    assert_eq!(cursor_distance((0, 0), (3, 10)), 21);
}

#[test]
fn units_show_through_when_selected_or_near_the_cursor() {
    assert!(see_through(true, (0, 0), (5000, 5000)));
    assert!(see_through(false, (0, 0), (255, 0)));
    assert!(!see_through(false, (0, 0), (256, 0)));
    assert!(!see_through(false, (0, 0), (0, 128)));
}

#[test]
fn the_sprite_frame_is_the_map_heading() {
    assert_eq!(openstrike::scene::frame_for(0), 0);
    assert_eq!(openstrike::scene::frame_for(17), 17);
    assert_eq!(openstrike::scene::frame_for(31), 31);
}
