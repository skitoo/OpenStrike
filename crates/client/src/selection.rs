//! Unit picking on screen, as pure functions over the drawn units.

use openstrike_sim::UnitId;

use crate::scene::UnitView;

/// Hit box of a unit around its ground position, in world-screen pixels: (left, top, right, bottom).
pub const HIT_BOX: (i32, i32, i32, i32) = (-24, -26, 24, 10);

/// Drags shorter than this (in world-screen pixels) are clicks.
pub const CLICK_SLOP: f32 = 4.0;

/// Entry of `user.pl` the selection rectangle is drawn in (`Game_Dll.dll` `0x10070499`,
/// `[0x10387d70]`): green.
pub const RECT_COLOUR: usize = 96;

/// The selection rectangle as the game draws it (`0x10070460`): from the press to the mouse, in
/// screen pixels, both corners included, as a one-pixel outline (`cad800.dll` `0x100023f0`); shown
/// from the press on, whatever its size. Returns x, y, width and height.
pub fn outline(a: (i32, i32), b: (i32, i32)) -> (i32, i32, i32, i32) {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

fn hit(u: &UnitView, x: f32, y: f32) -> bool {
    let (sx, sy) = (u.screen.0 as f32, u.screen.1 as f32);
    x >= sx + HIT_BOX.0 as f32 && x <= sx + HIT_BOX.2 as f32 && y >= sy + HIT_BOX.1 as f32 && y <= sy + HIT_BOX.3 as f32
}

/// The front-most unit under a point.
pub fn unit_at(units: &[UnitView], point: (f32, f32)) -> Option<UnitId> {
    units.iter().filter(|u| hit(u, point.0, point.1)).max_by_key(|u| u.screen.1).map(|u| u.id)
}

/// Units whose ground position lies in the rectangle spanned by two corners.
pub fn units_in(units: &[UnitView], a: (f32, f32), b: (f32, f32)) -> Vec<UnitId> {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    units
        .iter()
        .filter(|u| {
            let (x, y) = (u.screen.0 as f32, u.screen.1 as f32);
            x >= x0 && x <= x1 && y >= y0 && y <= y1
        })
        .map(|u| u.id)
        .collect()
}

/// New selection after a left-button release: a click picks one unit (or none), a drag picks a box.
/// With `add`, the result is merged into the current selection instead of replacing it.
pub fn select(current: &[UnitId], units: &[UnitView], press: (f32, f32), release: (f32, f32), add: bool) -> Vec<UnitId> {
    let dragged = (release.0 - press.0).abs().max((release.1 - press.1).abs()) >= CLICK_SLOP;
    let picked = if dragged { units_in(units, press, release) } else { unit_at(units, release).into_iter().collect() };
    let mut out = if add { current.to_vec() } else { Vec::new() };
    for id in picked {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}
