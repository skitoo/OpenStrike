//! What to draw, as plain values: the shell turns these into draw calls.

use openstrike_formats::iso::Projection;
use openstrike_formats::map::Map;
use openstrike_formats::objects::{Layer, ObjectSet, map_state};
use openstrike_sim::{SUBPIXEL, UnitId, World};

/// One part of a map object, in screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectPart {
    pub screen: (i32, i32),
    pub set: usize,
    pub frame: usize,
    /// State drawn (the map's state when decoded, else 0).
    pub state: usize,
    pub part: usize,
}

/// Map object parts sorted into the three layers. Upright parts are sorted back to front.
#[derive(Debug, Default)]
pub struct Objects {
    pub flat: Vec<ObjectPart>,
    pub shadows: Vec<ObjectPart>,
    pub upright: Vec<ObjectPart>,
}

pub fn objects(map: &Map, proj: &Projection, sets: &[ObjectSet]) -> Objects {
    let mut out = Objects::default();
    for o in &map.objects {
        let (set, frame) = (o.set as usize, o.frame as usize);
        let Some(object) = sets.get(set).and_then(|s| s.frames.get(frame)) else { continue };
        let name = map.object_sets.get(set).map_or("", String::as_str);
        let state = object.resolve(map_state(name, o.extra[0]));
        let screen = proj.to_screen(o.x as i32, o.y as i32);
        for (part, p) in object.parts(state).iter().enumerate() {
            let item = ObjectPart { screen, set, frame, state, part };
            match p.layer {
                Layer::Flat => out.flat.push(item),
                Layer::Shadow => out.shadows.push(item),
                Layer::Upright => out.upright.push(item),
            }
        }
    }
    out.upright.sort_by_key(|p| p.screen.1);
    out
}

/// A unit as drawn: screen position and the sprite frames of its hull and turret headings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitView {
    pub id: UnitId,
    /// Index into the vehicle types.
    pub kind: usize,
    pub screen: (i32, i32),
    pub frame: usize,
    pub turret_frame: usize,
}

/// Sprite frame for a map heading (0-31, clockwise from east): the heading is projected, then
/// matched to the 32 screen headings of the sprite strip (frame k faces (k + 4) * 11.25 degrees
/// clockwise from east). How the game picks the frame is not read in its code yet.
pub fn frame_for(heading: u8) -> usize {
    let angle = (heading as f32) * std::f32::consts::TAU / 32.0;
    let (dx, dy) = (angle.cos(), angle.sin());
    let (sx, sy) = (dx - dy, (dx + dy) / 2.0);
    let screen_heading = (sy.atan2(sx) / std::f32::consts::TAU * 32.0).round().rem_euclid(32.0) as u8;
    openstrike_formats::hot::frame_for_heading(screen_heading)
}

pub fn units(world: &World, proj: &Projection) -> Vec<UnitView> {
    world
        .units
        .iter()
        .map(|u| {
            let (x, y) = u.position();
            UnitView {
                id: u.id,
                kind: u.kind,
                screen: proj.to_screen(x.div_euclid(SUBPIXEL), y.div_euclid(SUBPIXEL)),
                frame: frame_for(u.facing),
                turret_frame: frame_for(u.turret),
            }
        })
        .collect()
}
