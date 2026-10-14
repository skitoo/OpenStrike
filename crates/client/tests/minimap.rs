use openstrike::minimap::{cell, dot, frame, origin_y, render, target, HEIGHT, ORIGIN_X, WIDTH};
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
        players: Vec::new(),
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

#[test]
fn the_view_frame_is_the_screen_scaled_down() {
    let m = map(128);
    // view whose top left is map pixel (64 * 32, 64 * 32): game screen (0, 2048), 800 x 600
    let pixels = frame(&m, (0, 64 * 32), (800, 600));
    // 25 x 18 pixels at widget (0, 64), all inside the rhomb
    let (x0, y0) = (ORIGIN_X, origin_y(&m) + 64);
    assert!(pixels.contains(&(x0, y0)));
    assert!(pixels.contains(&(x0 + 24, y0 + 17)));
    assert!(!pixels.contains(&(x0 + 25, y0)));
    assert!(!pixels.contains(&(x0 + 1, y0 + 1))); // hollow
    assert_eq!(pixels.len(), 2 * 25 + 2 * 18);
    // halved maps: 64 pixels per minimap pixel
    let pixels = frame(&map(256), (0, 64 * 64), (800, 600));
    assert!(pixels.contains(&(ORIGIN_X + 11, origin_y(&map(256)) + 64 + 8)));
    assert!(!pixels.contains(&(ORIGIN_X + 12, origin_y(&map(256)) + 64)));
}

#[test]
fn the_frame_is_only_drawn_on_the_widget() {
    let m = map(128);
    // top left corner of the map: cell (0, 0) is not the widget's, cell (4, 4) is
    let pixels = frame(&m, (-400, 0), (800, 600));
    let oy = origin_y(&m);
    assert!(!pixels.contains(&(ORIGIN_X, oy)));
    assert!(pixels.iter().all(|&(_, y)| y >= oy + 4));
    // the widget's pixels: cell (x, y) at (x - y, (x + y) >> 1); (5, 4) gives (1, 4), (4, 5) (-1, 4)
    let pixels = frame(&m, (-32, 4 * 32), (96, 32));
    assert!(pixels.contains(&(ORIGIN_X - 1, oy + 4)));
    assert!(pixels.contains(&(ORIGIN_X, oy + 4)));
    assert!(pixels.contains(&(ORIGIN_X + 1, oy + 4)));
    assert!(!pixels.contains(&(ORIGIN_X - 2, oy + 4))); // cell (3, 5)
}

#[test]
fn units_are_dots_at_their_cell() {
    let m = map(128);
    let oy = origin_y(&m);
    let square = |x: i32, y: i32| vec![(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
    // cell (10, 6): from pixel (10 - 6, (10 + 6) >> 1), anywhere in the cell
    assert_eq!(dot(&m, 10 * 32, 6 * 32), square(ORIGIN_X + 4, oy + 8));
    assert_eq!(dot(&m, 10 * 32 + 31, 6 * 32 + 31), square(ORIGIN_X + 4, oy + 8));
    // odd sums round down: cell (11, 6) is at (5, 8)
    assert_eq!(dot(&m, 11 * 32, 6 * 32)[0], (ORIGIN_X + 5, oy + 8));
    // not on the widget near the edge, cut by the rhomb's border
    assert!(dot(&m, 0, 60 * 32).is_empty());
    assert_eq!(dot(&m, 4 * 32, 4 * 32), vec![(ORIGIN_X, oy + 4), (ORIGIN_X + 1, oy + 4), (ORIGIN_X, oy + 5), (ORIGIN_X + 1, oy + 5)]);
    // halved maps: two cells per pixel
    let big = map(256);
    assert_eq!(dot(&big, 21 * 32, 13 * 32)[0], (ORIGIN_X + 4, origin_y(&big) + 8));
}
