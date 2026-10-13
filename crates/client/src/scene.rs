//! What to draw, as plain values: the shell turns these into draw calls.

use openstrike_formats::iso::Projection;
use openstrike_formats::map::Map;
use openstrike_formats::objects::{Layer, ObjectSet};
use openstrike_sim::{Direction, SUBPIXEL, UnitId, World};

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
        let state = object.resolve(o.extra[0] as usize);
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

/// A unit as drawn: screen position and the sprite frame of its heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitView {
    pub id: UnitId,
    pub kind: String,
    pub screen: (i32, i32),
    pub frame: usize,
}

/// Sprite frame for a map-space heading: the heading is projected, then matched to the 32 screen
/// headings of the sprite strip (frame k faces (k + 4) * 11.25 degrees clockwise from east).
pub fn frame_for(facing: Direction) -> usize {
    let angle = (facing.0 as f32) * std::f32::consts::TAU / Direction::COUNT as f32;
    let (dx, dy) = (angle.cos(), angle.sin());
    let (sx, sy) = (dx - dy, (dx + dy) / 2.0);
    let screen_heading = (sy.atan2(sx) / std::f32::consts::TAU * 32.0).round().rem_euclid(32.0) as u8;
    openstrike_formats::hot::frame_for_heading(screen_heading)
}

pub fn units(world: &World, proj: &Projection) -> Vec<UnitView> {
    world
        .units
        .iter()
        .map(|u| UnitView {
            id: u.id,
            kind: u.kind.clone(),
            screen: proj.to_screen(u.pos.x / SUBPIXEL, u.pos.y / SUBPIXEL),
            frame: frame_for(u.facing),
        })
        .collect()
}
