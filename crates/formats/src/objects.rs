//! Map object sets: `<set>.rs2` + `<set>.pl` in the climate archive (`dom` lives in `houses.rs2`).
//!
//! Each `.rs2` frame is one object: a set-specific header, then a run of sprites (colour, ground,
//! shadow, then the same for damaged or destroyed states).
//!
//! `tree` and `stand` frames carry a table of 6 u32 sprite offsets (0 = none), at byte 5 for `tree`
//! (loader `Game_Dll.dll` `0x10055890`) and at byte 205 for `stand`: colour intact, colour destroyed,
//! shadow intact, shadow destroyed, ground intact, ground destroyed (a tree's "ground destroyed" is
//! the fallen tree). The intact state is slots 0, 2 and 4. `stand`'s `.pl` holds 5 palettes and the
//! frame's first u32 (always 0-4) picks one.
//!
//! The other sets have no such table (`zabor`: u32 sprite length, loader `0x10056dc0`; `dom` and `most`:
//! their own 512-byte palette first). Their sprites are located with [`sprite::frame_len`] and the
//! intact state is every sprite up to and including the first shadow (the first sprite when there is
//! no shadow).

use crate::palette::Palette;
use crate::sprite::{self, Image, KIND_SHADOW};
use crate::{Error, Result};

/// Sets drawn flat on the ground (roads, rails, paths), under shadows and upright objects.
pub const FLAT_SETS: [&str; 6] = ["trop", "lay", "fre", "vor", "pont", "bred"];
/// Sets whose frames start with their own palette.
pub const OWN_PALETTE_SETS: [&str; 2] = ["dom", "most"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    /// Painted on the ground, under everything else.
    Flat,
    Shadow,
    /// Stands up from the ground: drawn back to front with the units.
    Upright,
}

#[derive(Debug, Clone)]
pub struct Part {
    pub layer: Layer,
    pub image: Image,
}

/// The intact state of every frame of a set; an empty list for empty or undecodable frames.
#[derive(Debug, Clone, Default)]
pub struct ObjectSet {
    pub frames: Vec<Vec<Part>>,
}

/// Archive file name (without extension) of a set name used in maps.
pub fn file_stem(set: &str) -> &str {
    if set.eq_ignore_ascii_case("dom") { "houses" } else { set }
}

/// Byte offset of the 6-slot sprite table, for the sets that have one.
fn slot_table(name: &str) -> Option<usize> {
    match name.to_ascii_lowercase().as_str() {
        "tree" => Some(5),
        "stand" => Some(205),
        _ => None,
    }
}

/// Intact sprites from the slot table: (offset, layer) for slots 0, 2 and 4.
fn slotted_sprites(frame: &[u8], table: usize) -> Vec<(usize, Layer)> {
    [(0, Layer::Upright), (2, Layer::Shadow), (4, Layer::Flat)]
        .into_iter()
        .filter_map(|(slot, layer)| {
            let at = crate::reader::u32_at(frame, table + 4 * slot).ok()? as usize;
            (at != 0 && sprite::frame_len(frame, at).is_some()).then_some((at, layer))
        })
        .collect()
}

fn intact_sprites(frame: &[u8], start: usize) -> Vec<(usize, u8)> {
    let mut found = Vec::new();
    let mut at = start;
    while at + 9 <= frame.len() {
        match sprite::frame_len(frame, at) {
            Some(len) => {
                let kind = frame[at + 8];
                found.push((at, kind));
                if kind == KIND_SHADOW {
                    return found;
                }
                at += len;
            }
            None => at += 1,
        }
    }
    found.truncate(1);
    found
}

/// Decode the intact state of every frame of a set. `palettes` (the set's `.pl`) is required unless
/// the set embeds its own; with several palettes, each frame's first u32 selects one.
pub fn load_set(name: &str, rs2: &[u8], palettes: &[Palette]) -> Result<ObjectSet> {
    let flat = FLAT_SETS.iter().any(|s| s.eq_ignore_ascii_case(name));
    let own_palette = OWN_PALETTE_SETS.iter().any(|s| s.eq_ignore_ascii_case(name));
    if !own_palette && palettes.is_empty() {
        return Err(Error::Invalid(format!("object set {name} needs a palette")));
    }
    let frames = sprite::rs2_frames(rs2)?
        .into_iter()
        .map(|frame| {
            let (pal, start) = if own_palette {
                match Palette::parse(frame) {
                    Ok(p) => (p, 512),
                    Err(_) => return Vec::new(),
                }
            } else {
                let select = match palettes.len() {
                    1 => 0,
                    _ => frame.get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()) as usize),
                };
                (palettes.get(select).unwrap_or(&palettes[0]).clone(), 0)
            };
            let sprites: Vec<(usize, Layer)> = match slot_table(name) {
                Some(table) => slotted_sprites(frame, table),
                None => intact_sprites(frame, start)
                    .into_iter()
                    .map(|(at, kind)| {
                        let layer = match kind {
                            KIND_SHADOW => Layer::Shadow,
                            0xa2 => Layer::Flat,
                            _ if flat => Layer::Flat,
                            _ => Layer::Upright,
                        };
                        (at, layer)
                    })
                    .collect(),
            };
            sprites
                .into_iter()
                .filter_map(|(at, layer)| Some(Part { layer, image: sprite::decode(&frame[at..], &pal).ok()? }))
                .collect()
        })
        .collect();
    Ok(ObjectSet { frames })
}
