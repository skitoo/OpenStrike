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
/// Horizontal origin of the minimap widget in the panel: entry 10 of the panel's layout
/// (`intrpic.dat` frame 0, 16-byte records, read at `0x1006f5b9`), (132, 6); the 6 is not used.
pub const ORIGIN_X: i32 = 132;
/// Entry of `user.pl` the view frame is drawn in (`[0x10387d72]`, the palette being loaded at
/// `0x10387cb0` by `0x10082240`): white, green in winter.
pub const FRAME_COLOUR: usize = 97;
/// Entry of `user.pl` of player 0's units; player `p` uses the next `p`-th (`0x1000f690`, the
/// tank's vtable `+0x244`, reads `[0x10387d30 + 2 * player]`).
pub const PLAYER_COLOURS: usize = 64;
/// Side of a unit's dot in minimap pixels (1 in the game, see [`dot`]).
pub const DOT_SIZE: i32 = 2;
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

/// Whether the minimap widget owns widget pixel `(px, py)`, the only pixels its lines are drawn
/// on (`0x1006e4c0`, `0x1006e540`). Its init (`0x1006e640`) marks one pixel per preview cell
/// `(x, y)` with `4 <= x < (width - 4) >> halved` and the same for `y`: pixel
/// `(x - y, (x + y) >> 1)`.
fn owned(map: &Map, px: i32, py: i32) -> bool {
    let s = halved(map) as u32;
    let (w, h) = ((map.width as i32 - 4) >> s, (map.height as i32 - 4) >> s);
    // the one cell landing there: x - y = px and x + y = 2 py + (px & 1)
    let x = py + ((px + 1) >> 1);
    let y = x - px;
    (4..w).contains(&x) && (4..h).contains(&y)
}

/// The frame of the view on the minimap (`0x1006eb8f`-`0x1006ed7f`), as panel pixels. `view` is
/// the top left of the view in the game's screen coordinates (map pixel `(x, y)` at `x - y`,
/// `(x + y) / 2`) and `size` its size, both scaled down by 32 (64 for halved maps) with arithmetic
/// shifts, the minimap being the game's screen at that scale. The outline is drawn as
/// `0x1006ee80` does, top and bottom rows then left and right columns, each pixel only where the
/// widget owns it.
pub fn frame(map: &Map, view: (i32, i32), size: (i32, i32)) -> Vec<(i32, i32)> {
    let s = 5 + halved(map) as u32;
    let (x0, y0, w, h) = (view.0 >> s, view.1 >> s, size.0 >> s, size.1 >> s);
    let rows = [y0, y0 + h - 1].into_iter().flat_map(|y| (x0..x0 + w).map(move |x| (x, y)));
    let columns = [x0, x0 + w - 1].into_iter().flat_map(|x| (y0..y0 + h).map(move |y| (x, y)));
    let oy = origin_y(map);
    rows.chain(columns).filter(|&(x, y)| owned(map, x, y)).map(|(x, y)| (x + ORIGIN_X, y + oy)).collect()
}

/// Panel pixels of the dot of a unit at map pixel `(x, y)`. After the frame, the game walks the
/// unit list (`0x1006ed84`) and calls each unit's vtable `+0x244` with the minimap pixel of its map
/// pixel shifted right by 5 (6 for halved maps); a tank (`0x1000f690`) puts one pixel there
/// (`0x1006e470`) when it is alive and seen by the local player (`0x100333d0`). **Accepted
/// difference**: OpenStrike draws [`DOT_SIZE`] pixels square from there, the single pixel being
/// too small to see on today's screens; each still only where the widget owns it.
pub fn dot(map: &Map, x: i32, y: i32) -> Vec<(i32, i32)> {
    let s = 5 + halved(map) as u32;
    let (x, y) = (x >> s, y >> s);
    let (px, py) = (x - y, (x + y) >> 1);
    let oy = origin_y(map);
    (0..DOT_SIZE)
        .flat_map(|dy| (0..DOT_SIZE).map(move |dx| (px + dx, py + dy)))
        .filter(|&(x, y)| owned(map, x, y))
        .map(|(x, y)| (x + ORIGIN_X, y + oy))
        .collect()
}
