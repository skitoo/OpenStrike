//! What to draw, as plain values: the shell turns these into draw calls.

use openstrike_formats::iso::Projection;
use openstrike_formats::map::Map;
use openstrike_formats::objects::{LAYER_SHADOW, LAYER_UPRIGHT, ObjectSet, fence_lies_flat, ground_layer, map_state, placement};
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
    /// Layer it is drawn on (0-7, as in the game).
    pub layer: u8,
    /// Screen y used to sort a layer back to front (the house halves add their depth).
    pub sort_y: i32,
}

/// Map object parts by layer, each sorted as the game's draw lists are (`Game_Dll.dll`
/// `0x1006f740`: sort y, then y, then x).
#[derive(Debug, Default)]
pub struct Objects {
    /// Layers 0-5, in layer order.
    pub ground: Vec<ObjectPart>,
    pub shadows: Vec<ObjectPart>,
    pub upright: Vec<ObjectPart>,
}

pub fn objects(map: &Map, proj: &Projection, sets: &[ObjectSet]) -> Objects {
    let mut out = Objects::default();
    for o in &map.objects {
        let (set, frame) = (o.set as usize, o.frame as usize);
        let Some(object) = sets.get(set).and_then(|s| s.frames.get(frame)) else { continue };
        let name = map.object_sets.get(set).map_or("", String::as_str);
        let Some((px, py)) = placement(name, o.x, o.y, map.width, map.height) else { continue };
        let state = object.resolve(map_state(name, o.extra[0]));
        let screen = proj.to_screen(px, py);
        let flat_fence = name.eq_ignore_ascii_case("zabor") && fence_lies_flat(o.extra);
        for (part, p) in object.parts(state).iter().enumerate() {
            let layer = if flat_fence && p.layer == LAYER_UPRIGHT { ground_layer(name) } else { p.layer };
            let item = ObjectPart { screen, set, frame, state, part, layer, sort_y: screen.1 + p.depth };
            match layer {
                LAYER_UPRIGHT => out.upright.push(item),
                LAYER_SHADOW => out.shadows.push(item),
                _ => out.ground.push(item),
            }
        }
    }
    out.ground.sort_by_key(ObjectPart::key);
    out.shadows.sort_by_key(ObjectPart::key);
    out.upright.sort_by_key(ObjectPart::key);
    out
}

impl ObjectPart {
    /// Draw order: layer, then the draw list order (sort y, y, x).
    pub fn key(&self) -> (u8, i32, i32, i32) {
        (self.layer, self.sort_y, self.screen.1, self.screen.0)
    }
}

/// A unit as drawn: screen position and the sprite frames of its hull and turret headings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitView {
    pub id: UnitId,
    /// Index into the vehicle types.
    pub kind: usize,
    pub player: u8,
    pub screen: (i32, i32),
    pub frame: usize,
    pub turret_frame: usize,
}

impl UnitView {
    /// Depth of the unit: its screen y. The game files its sprites in buckets by this value and
    /// draws them in increasing order (`Game_Dll.dll` `0x10070d00`).
    pub fn depth(&self) -> i32 {
        self.screen.1
    }
}

/// Distance the game uses between a unit and the mouse cursor (`Game_Dll.dll` `0x10003550` on
/// `dx` and `2 * dy` in screen pixels): the larger plus half the smaller, halves truncated.
pub fn cursor_distance(unit: (i32, i32), mouse: (i32, i32)) -> i32 {
    let (a, b) = ((unit.0 - mouse.0).abs(), (2 * (unit.1 - mouse.1)).abs());
    if a > b { a + b / 2 } else { b + a / 2 }
}

/// Whether a unit shows through the upright objects in front of it, on every other pixel: when it
/// is selected or within 256 of the cursor (`Game_Dll.dll` `0x1000ec1b`, bit 13 of `+0xdb`; the
/// blitter `cad800.dll` `0x100072a2` then skips the depth test on a checkerboard).
pub fn see_through(selected: bool, unit: (i32, i32), mouse: (i32, i32)) -> bool {
    selected || cursor_distance(unit, mouse) < 256
}

/// Sprite frame for a map heading (0-31, clockwise from the map's east): the heading itself, for
/// the hull (`Game_Dll.dll` `0x1000eb7b`, from `+0x63`) and the turret (`0x1000eb99`, from `+0xce`).
pub fn frame_for(heading: u8) -> usize {
    (heading & 31) as usize
}

/// The units a side sees (`side_bit` = `1 << side`): those whose cell it sees, its own included
/// since their sight covers their cell (the tank's vtable `+0x74`, `0x100333d0`).
pub fn seen_units(world: &World, proj: &Projection, side_bit: u8) -> Vec<UnitView> {
    let seen: Vec<bool> = world.units.iter().map(|u| u.seen_by & side_bit != 0).collect();
    units(world, proj).into_iter().zip(seen).filter(|(_, s)| *s).map(|(v, _)| v).collect()
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
                player: u.player,
                screen: proj.to_screen(x.div_euclid(SUBPIXEL), y.div_euclid(SUBPIXEL)),
                frame: frame_for(u.facing),
                turret_frame: frame_for(u.turret),
            }
        })
        .collect()
}
