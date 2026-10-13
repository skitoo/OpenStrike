//! Map object sets: `<set>.rs2` + `<set>.pl` in the climate archive (`dom` lives in `houses.rs2`).
//!
//! Each `.rs2` frame is one object: a set-specific header, then a run of sprites (colour, ground,
//! shadow, then the same for damaged or destroyed states). The headers are only partly understood
//! (`zabor`: u32 sprite length; `tree`: 5 bytes then 6 u32 sprite offsets, from the loaders at
//! `Game_Dll.dll` `0x10056dc0` and `0x10055890`; `dom` and `most`: their own 512-byte palette first;
//! `stand`: its `.pl` holds 5 palettes and the frame's first u32 (always 0-4) picks one),
//! so the sprites are located with [`sprite::frame_len`] and the intact state is taken as every
//! sprite up to and including the first shadow (only the first sprite when there is no shadow).

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
            intact_sprites(frame, start)
                .into_iter()
                .filter_map(|(at, kind)| {
                    let image = sprite::decode(&frame[at..], &pal).ok()?;
                    let layer = match kind {
                        KIND_SHADOW => Layer::Shadow,
                        0xa2 => Layer::Flat,
                        _ if flat => Layer::Flat,
                        _ => Layer::Upright,
                    };
                    Some(Part { layer, image })
                })
                .collect()
        })
        .collect();
    Ok(ObjectSet { frames })
}
