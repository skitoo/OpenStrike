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

/// The fields of the bar for a unit under the mouse (`0x100331c0`, the tank's vtable `+0xa0`, a
/// type 1 record): hit points, crew, then both ammunitions (only for units of a friendly side,
/// `[0x101ff3ac + 2 * local]`). Each is drawn (`0x100781c0`) as its icon (`intrpic.dat` frame
/// `7 + field` on the bar's layout record `2 * field + 3`), a gauge on record `2 * field + 4` and the
/// value centred under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub value: String,
    /// Percentage shown by the gauge.
    pub percent: i32,
    /// Percentage of the crew's own condition, drawn over the crew gauge (all of it unless veterans
    /// raise the gauge).
    pub under: Option<i32>,
}

/// Gauge colours, `user.pl` entries, by field; the crew's own condition is entry 8.
pub const GAUGE_COLOURS: [usize; 4] = [2, 7, 3, 3];
pub const CREW_COLOUR: usize = 8;

/// A unit's fields. `crew_shown` is the crew condition the status bars show (raised for veterans).
pub fn unit_fields(
    hp: i32,
    max_hp: i32,
    crew: i32,
    crew_shown: i32,
    ammo: [i32; 2],
    max_ammo: [i32; 2],
    friendly: bool,
) -> [Option<Field>; 4] {
    let hp_percent = (hp * 100 / max_hp).max(1);
    let shown = (crew_shown * 1000) >> 15;
    let own = (crew * 1000) >> 15;
    let crew_field = Field {
        value: if own != shown { format!("{own}/{shown}") } else { shown.to_string() },
        percent: shown * 100 / 1000,
        under: Some(own * 100 / 1000),
    };
    let ammo_field = |i: usize| {
        (friendly && max_ammo[i] != 0).then(|| Field {
            value: ammo[i].to_string(),
            percent: ammo[i] * 100 / max_ammo[i],
            under: None,
        })
    };
    [
        Some(Field { value: hp.to_string(), percent: hp_percent, under: None }),
        Some(crew_field),
        ammo_field(0),
        ammo_field(1),
    ]
}

/// Length of a gauge `width` pixels wide for `percent` (`0x10077fb0`).
pub fn gauge(width: i32, percent: i32) -> i32 {
    if percent > 0 { width * percent / 100 } else { 0 }
}

/// Like [`layout`], centred on 0 (`0x1005b100` with alignment 2: the start moves back by half the
/// total advance).
pub fn layout_centred(text: &str, width: impl Fn(u8) -> i32) -> Vec<(i32, u8)> {
    let glyphs = layout(text, &width);
    let total: i32 = text.chars().map(|c| width(u32::from(c).min(255) as u8) + SPACING).sum();
    glyphs.into_iter().map(|(x, c)| (x - (total >> 1), c)).collect()
}
