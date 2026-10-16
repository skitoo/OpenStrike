//! Unit picking on screen, as pure functions over the drawn units.

use std::collections::HashMap;

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
pub fn select(
    current: &[UnitId],
    units: &[UnitView],
    press: (f32, f32),
    release: (f32, f32),
    add: bool,
) -> Vec<UnitId> {
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

/// The group of each unit that has one, 1-10 (the unit's `+0x40`). The digit keys (`key.map`
/// `key_g0`-`key_g9`: 0-9) stand for groups 1-10.
pub type Groups = HashMap<UnitId, u8>;

/// Seconds within which pressing the same group key again centres the view on it: the system's
/// double-click time (`GetDoubleClickTime`, 500 ms by default).
pub const RECALL_TWICE: f32 = 0.5;

/// Ctrl + group key (`Game_Dll.dll` `0x100837b0`): the selected units join group `g`; with `keep`
/// (Shift held too) the group's other units stay in it, otherwise they leave it. `units` are all
/// the units, in the game's list order.
pub fn assign(groups: &mut Groups, units: &[UnitId], selected: &[UnitId], g: u8, keep: bool) {
    for id in units {
        if selected.contains(id) {
            groups.insert(*id, g);
        } else if !keep && groups.get(id) == Some(&g) {
            groups.remove(id);
        }
    }
}

/// Group key (`0x10083400`): the units of group `g` are selected; without `add` (Shift) the
/// selected units outside the group are dropped. Then, if any selected unit is `local`'s, the
/// others' are dropped. `units` are the units that may be selected (alive and seen, `+0x74`), in
/// list order, which is the order of the result.
pub fn recall(units: &[UnitView], selected: &[UnitId], groups: &Groups, g: u8, add: bool, local: u8) -> Vec<UnitId> {
    let in_group = |id: &UnitId| groups.get(id) == Some(&g);
    let picked: Vec<&UnitView> =
        units.iter().filter(|u| in_group(&u.id) || (add && selected.contains(&u.id))).collect();
    let own = picked.iter().any(|u| u.player == local);
    picked.into_iter().filter(|u| !own || u.player == local).map(|u| u.id).collect()
}

/// Map pixel the view centres on when a group key is pressed twice: the mean of the selected
/// units' cells, times 32 (`0x10083400`, integer division of the sums).
pub fn centre(cells: &[(i16, i16)]) -> Option<(i32, i32)> {
    let n = cells.len() as i32;
    if n == 0 {
        return None;
    }
    let sum = cells.iter().fold((0, 0), |s, c| (s.0 + c.0 as i32, s.1 + c.1 as i32));
    Some(((sum.0 << 5) / n, (sum.1 << 5) / n))
}
