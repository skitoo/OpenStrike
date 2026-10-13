use openstrike_formats::rhomb::{RHOMB_BYTES, RHOMB_HEIGHT, cell_centre, row_width};

#[test]
fn rhomb_rows_add_up_to_1024_pixels() {
    assert_eq!((0..RHOMB_HEIGHT).map(row_width).sum::<i32>(), RHOMB_BYTES as i32);
    assert_eq!(row_width(0), 2);
    assert_eq!(row_width(15), 62);
    assert_eq!(row_width(16), 62);
    assert_eq!(row_width(31), 2);
}

#[test]
fn odd_columns_are_shifted_half_a_rhomb_down() {
    assert_eq!(cell_centre(0, 0), (0, 0));
    assert_eq!(cell_centre(1, 0), (32, 16));
    assert_eq!(cell_centre(2, 1), (64, 32));
}
