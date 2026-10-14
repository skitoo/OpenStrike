//! The information bar at the bottom of the screen (`Game_Dll.dll` `0x10077af0`, the `NWTT` window):
//! each frame the window under the mouse is asked what it shows (`0x1006d2a4`, the window's vtable
//! `+0x40`, `0x10075480`: a button's text of `intrtext.dat`), and the bar shows that text
//! (`0x10077c30`) for 200 ms after the last time it was given.

use std::collections::HashMap;

/// The bar's picture (`intrpic.dat` frame 5, 368 x 32) ends at this x on the screen, at the bottom; it
/// fills the notch of the panel picture (transparent from (272, 104)).
pub const BAR_RIGHT: i32 = 639;
/// Where the text starts in the bar: the bar's layout (`intrpic.dat` frame 4) record 2, the label
/// itself being record 1, (9, 8)-(336, 18) (`0x10074740`).
pub const TEXT_AT: (i32, i32) = (11, 16);
/// Pixels between two glyphs (the label's `+0x39`, `0x1005b100`).
pub const SPACING: i32 = 2;
/// How long a text stays after the mouse left what gave it.
pub const HOLD_SECONDS: f64 = 0.2;

/// Texts of `intrtext.dat` for TENIR LA POSITION and SUSPENDRE LE FEU (tooltips 8, 9 + 1).
pub const HOLD_TEXTS: [usize; 2] = [9, 10];

/// The text with each `~key_xx~` replaced by the name of the key bound to `xx` (`0x10077d90`),
/// nothing when there is none.
pub fn expand(text: &str, key_names: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut parts = text.split('~');
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, part) in parts.enumerate() {
        if i % 2 == 0 {
            if let Some(name) = part.strip_prefix("key_").and_then(|a| key_names.get(&a.to_ascii_lowercase())) {
                out.push_str(name);
            }
        } else {
            out.push_str(part);
        }
    }
    out
}

/// The x of each character from the text's start, left-aligned (`0x1005b100`): each one advances by
/// its glyph's width (0 without one) and [`SPACING`].
pub fn layout(text: &str, width: impl Fn(u8) -> i32) -> Vec<(i32, u8)> {
    let mut x = 0;
    text.chars()
        .map(|c| {
            let code = u32::from(c).min(255) as u8;
            let at = (x, code);
            x += width(code) + SPACING;
            at
        })
        .collect()
}
