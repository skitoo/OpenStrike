use openstrike_formats::iso::Projection;
use openstrike_formats::rhomb::{RHOMB_BYTES, RHOMB_HEIGHT, row_width};

#[test]
fn rhomb_rows_add_up_to_1024_pixels() {
    assert_eq!((0..RHOMB_HEIGHT).map(row_width).sum::<i32>(), RHOMB_BYTES as i32);
    assert_eq!(row_width(0), 2);
    assert_eq!(row_width(15), 62);
    assert_eq!(row_width(16), 62);
    assert_eq!(row_width(31), 2);
}

#[test]
fn a_cell_projects_onto_a_64x32_rhomb() {
    let p = Projection { width: 128, height: 128 };
    let (top_x, top_y) = p.to_screen(0, 0);
    let (right_x, right_y) = p.to_screen(32, 0);
    let (left_x, left_y) = p.to_screen(0, 32);
    let (bottom_x, bottom_y) = p.to_screen(32, 32);
    assert_eq!((right_x - left_x, bottom_y - top_y), (64, 32));
    assert_eq!((right_y, left_y), (16, 16));
    assert_eq!(bottom_x, top_x);
    assert_eq!(p.cell_centre(0, 0), (top_x, 16));
}

#[test]
fn screen_to_map_inverts_the_projection() {
    let p = Projection { width: 128, height: 256 };
    for (x, y) in [(0, 0), (100, 40), (4095, 8191), (3000, 17)] {
        let (sx, sy) = p.to_screen(x, y);
        let (mx, my) = p.to_map(sx as f32, sy as f32);
        assert!((mx - x as f32).abs() <= 1.0 && (my - y as f32).abs() <= 1.0, "{x},{y} -> {mx},{my}");
    }
    assert_eq!(p.screen_size(), (12288, 6144));
}
