use openstrike_formats::iso::Projection;
use openstrike_formats::rhomb::{RHOMB_BYTES, RHOMB_HEIGHT, row_start, row_width, shade};

#[test]
fn rhomb_rows_add_up_to_1024_pixels() {
    assert_eq!((0..RHOMB_HEIGHT).map(row_width).sum::<i32>(), RHOMB_BYTES as i32);
    assert_eq!(row_width(0), 3);
    assert_eq!(row_width(15), 63);
    assert_eq!(row_width(16), 61);
    assert_eq!(row_width(31), 1);
    // every row is centred on the top vertex's column
    for row in 0..RHOMB_HEIGHT {
        assert_eq!(row_start(row) + row_width(row) - 1, -row_start(row), "row {row}");
    }
}

/// Pixel index of `(row, i)` in a rhomb.
fn at(row: i32, i: i32) -> usize {
    ((0..row).map(row_width).sum::<i32>() + i) as usize
}

#[test]
fn brightness_is_interpolated_between_the_four_corners() {
    let flat: Vec<_> = shade(9, 9, 9, 9, false).collect();
    assert_eq!(flat.len(), RHOMB_BYTES);
    assert!(flat.iter().all(|&(level, _)| level == 9));

    let s: Vec<_> = shade(0, 16, 8, 31, false).map(|(level, _)| level).collect();
    assert_eq!(s[at(0, 1)], 0); // top vertex: the cell
    assert_eq!(s[at(15, 0)], 8); // left vertex: y + 1
    assert_eq!(s[at(15, 62)], 15); // right vertex: x + 1, one step short
    assert_eq!(s[at(16, 60)], 16); // 8 + 23 / 16 + 8 * 60 / 64
    assert_eq!(s[at(31, 0)], 31); // bottom vertex: (x + 1, y + 1)
}

#[test]
fn the_two_palette_ramps_alternate_in_a_checkerboard() {
    let odd: Vec<_> = shade(5, 5, 5, 5, false).map(|(_, odd)| odd).collect();
    assert_eq!(&odd[at(0, 0)..at(0, 3)], &[false, true, false]);
    assert_eq!(&odd[at(1, 0)..at(1, 3)], &[true, false, true]);
    // the row after the middle starts 3 pixels to the right of the previous one, keeping the board
    assert!(odd[at(15, 0)]);
    assert!(odd[at(16, 0)]);
    assert!(shade(5, 5, 5, 5, true).zip(&odd).all(|((_, a), &b)| a != b));
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
