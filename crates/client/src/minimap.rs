//! The original interface panel's minimap (`Game_Dll.dll` `0x1006eef0`, set up by `0x1006e5c0`):
//! widget pixel `(px, py)`, `px` signed around the widget's origin, shows preview cell
//! `(py + px / 2, py - px / 2)` (halves by arithmetic shift), when that cell is at least 4 cells
//! from the edge; other pixels are left to the panel picture (its black rhomb). The preview is the
//! map's own minimap (`XCHNG\TOGAME\map_mini`, one RGB565 colour per cell, halved for maps wider
//! or higher than 128 cells, which also halves the minimap's scale: `+0x25`).

use openstrike_formats::map::Map;
use openstrike_formats::palette::rgb565;

/// The panel picture (`intrpic.dat` frame 1, 352 x 136) and where the game puts it: the bottom
/// left corner of the screen (`0x1006f540`).
pub const PANEL_SIZE: (i32, i32) = (352, 136);
/// Horizontal origin of the minimap widget in the panel. The game takes it from a panel resource
/// value not traced yet (`0x1006f5c8`); 132 is the middle column of the panel picture's black
/// rhomb, which the formula fills exactly.
pub const ORIGIN_X: i32 = 132;
/// Size of the minimap image, covering `px` in `-ORIGIN_X..WIDTH - ORIGIN_X`.
pub const WIDTH: i32 = 264;
pub const HEIGHT: i32 = 136;

/// Whether the map uses the halved preview (and minimap scale).
pub fn halved(map: &Map) -> bool {
    map.width > 128 || map.height > 128
}

/// Vertical origin of the widget in the panel, centring the rhomb (`0x1006e5c0`):
/// `(bottom - (width + height) / 2 - top + 1) >> 1` in the panel, `/ 4` for halved maps.
pub fn origin_y(map: &Map) -> i32 {
    let sum = (map.width + map.height) as i32;
    let rhomb = if halved(map) { sum >> 2 } else { sum >> 1 };
    (PANEL_SIZE.1 - 1 - rhomb + 1) >> 1
}

/// Preview cell under widget pixel `(px, py)`.
pub fn cell(px: i32, py: i32) -> (i32, i32) {
    (py + (px >> 1), py - (px >> 1))
}

/// Whether the game draws that cell: at least 4 cells from the edge, compared with the map's size
/// in cells as the game does, and inside the preview.
fn shown(map: &Map, (x, y): (i32, i32)) -> bool {
    let (pw, ph) = (map.preview_size.0 as i32, map.preview_size.1 as i32);
    x >= 4 && y >= 4 && x < map.width as i32 - 4 && y < map.height as i32 - 4 && x < pw && y < ph
}

/// The minimap as RGBA8, `WIDTH x HEIGHT`, row 0 at the panel's row 0; transparent where the
/// panel shows through. Every cell is drawn as seen (the game darkens unseen ones, `0x1006f04b`;
/// there is no fog of war yet).
pub fn render(map: &Map) -> Vec<u8> {
    let mut out = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    let oy = origin_y(map);
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            let (px, py) = (col - ORIGIN_X, row - oy);
            if py < 0 {
                continue;
            }
            let c = cell(px, py);
            if shown(map, c) {
                let rgb = rgb565(map.preview[(c.1 * map.preview_size.0 as i32 + c.0) as usize]);
                out[((row * WIDTH + col) * 4) as usize..][..4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    out
}

/// The map pixel to centre the view on for a click at panel pixel `(x, y)`, if it is on the
/// minimap (`0x1006f3f2`: the view moves to the cell clicked, scaled back for halved maps).
pub fn target(map: &Map, x: i32, y: i32) -> Option<(i32, i32)> {
    let c = cell(x - ORIGIN_X, y - origin_y(map));
    if y < origin_y(map) || !shown(map, c) {
        return None;
    }
    let scale = if halved(map) { 64 } else { 32 };
    Some((c.0 * scale + scale / 2, c.1 * scale + scale / 2))
}
