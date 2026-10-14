use openstrike::minimap::{cell, origin_y, render, target, HEIGHT, ORIGIN_X, WIDTH};
use openstrike_formats::map::Map;

fn map(size: u32) -> Map {
    let preview = if size > 128 { size / 2 } else { size };
    Map {
        version: 0x10f,
        header: [0; 5],
        width: size,
        height: size,
        briefing: String::new(),
        preview_size: (preview, preview),
        preview: vec![0xffff; (preview * preview) as usize],
        ground: Vec::new(),
        layer2: Vec::new(),
        object_sets: Vec::new(),
        objects: Vec::new(),
        unit_types: Vec::new(),
        units: Vec::new(),
    }
}

#[test]
fn widget_pixels_map_to_cells_as_the_game_shifts() {
    assert_eq!(cell(0, 10), (10, 10));
    assert_eq!(cell(4, 10), (12, 8));
    assert_eq!(cell(-3, 10), (8, 12)); // -3 >> 1 == -2
}

#[test]
fn the_rhomb_is_centred_in_the_panel() {
    assert_eq!(origin_y(&map(128)), 4);
    assert_eq!(origin_y(&map(256)), 4);
    assert_eq!(origin_y(&map(64)), 36);
}

#[test]
fn border_cells_leave_the_panel_visible() {
    let m = map(128);
    let rgba = render(&m);
    let alpha = |x: i32, y: i32| rgba[((y * WIDTH + x) * 4 + 3) as usize];
    assert_eq!(alpha(ORIGIN_X, 4), 0); // cell (0, 0)
    assert_eq!(alpha(ORIGIN_X, 4 + 64), 255); // cell (64, 64)
    assert_eq!(rgba.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_eq!(target(&m, ORIGIN_X, 4 + 64), Some((64 * 32 + 16, 64 * 32 + 16)));
    assert_eq!(target(&map(256), ORIGIN_X, 4 + 64), Some((64 * 64 + 32, 64 * 64 + 32)));
    assert_eq!(target(&m, ORIGIN_X, 4), None);
}
