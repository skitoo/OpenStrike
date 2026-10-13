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
//! no shadow), or the second shadow for bridges, whose first sprite is only the abutments and the
//! second the deck.

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

/// Side of a building footprint grid, in tiles of 16 map pixels.
pub const FOOTPRINT_SIZE: usize = 14;
/// Grid cell under the object position. Fitted on the data: it best aligns the solid cells with the
/// house's ground sprite for 75 of the 113 summer houses (7 for 5 of them, 5 for 31).
pub const FOOTPRINT_ORIGIN: i32 = 6;
pub type Footprint = [[u8; FOOTPRINT_SIZE]; FOOTPRINT_SIZE];

/// Footprint cells that block vehicles: bits 0x40 (the building) and 0x04 (its walls).
/// Inferred from the data: these bits form the solid block, while 0x01-0x03 ring it.
pub const FOOTPRINT_SOLID: u8 = 0x44;

/// Side of a bridge grid (`most`), in tiles of 16 map pixels.
pub const BRIDGE_SIZE: usize = 22;
/// Bridge grid tile under the object position. Fitted on the 723 bridges of the summer single-player
/// maps: it puts the most deck tiles over water (44 %, the rest are the ramps) while keeping the
/// abutments (0x08, 0x09) on land (82 %); (4, 3) scores the same.
pub const BRIDGE_ORIGIN: (i32, i32) = (4, 4);
/// Bridge grid values with these bits set are the deck: 0xff, 0x7f, 0x3f, 0xfc, 0xfe (inferred).
pub const BRIDGE_DECK: u8 = 0x30;

/// The intact state of every frame of a set; an empty list for empty or undecodable frames.
#[derive(Debug, Clone, Default)]
pub struct ObjectSet {
    pub frames: Vec<Vec<Part>>,
    /// Building footprints (`dom` only), per frame.
    pub footprints: Vec<Option<Footprint>>,
    /// Deck tiles of bridges (`most` only), per frame, as (x, y) in the bridge grid.
    pub decks: Vec<Option<Vec<(i32, i32)>>>,
}

/// A bridge frame: 512-byte palette, two u16, then the 22x22 grid ending at byte 1000, where a table
/// of sprite offsets (relative to byte 1000) starts. Starting the grid at 516 is the only offset that
/// leaves no deck tile in the last column, which would otherwise wrap onto the next row.
fn bridge_deck(frame: &[u8]) -> Option<Vec<(i32, i32)>> {
    let raw = frame.get(516..516 + BRIDGE_SIZE * BRIDGE_SIZE)?;
    Some(
        raw.iter()
            .enumerate()
            .filter(|&(_, &v)| v & BRIDGE_DECK != 0)
            .map(|(i, _)| ((i % BRIDGE_SIZE) as i32, (i / BRIDGE_SIZE) as i32))
            .collect(),
    )
}

/// A house frame: 512-byte palette, u16, u32, then the 14x14 footprint grid
/// (read by the loader at `Game_Dll.dll` `0x10050980`).
fn house_footprint(frame: &[u8]) -> Option<Footprint> {
    let raw = frame.get(518..518 + FOOTPRINT_SIZE * FOOTPRINT_SIZE)?;
    let mut grid = [[0u8; FOOTPRINT_SIZE]; FOOTPRINT_SIZE];
    for (row, chunk) in grid.iter_mut().zip(raw.as_chunks::<FOOTPRINT_SIZE>().0) {
        *row = *chunk;
    }
    Some(grid)
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

/// Sprites up to and including the `shadows`-th shadow (only the first sprite if there is none).
fn intact_sprites(frame: &[u8], start: usize, shadows: usize) -> Vec<(usize, u8)> {
    let mut found = Vec::new();
    let mut seen = 0;
    let mut at = start;
    while at + 9 <= frame.len() {
        match sprite::frame_len(frame, at) {
            Some(len) => {
                let kind = frame[at + 8];
                found.push((at, kind));
                if kind == KIND_SHADOW {
                    seen += 1;
                    if seen == shadows {
                        return found;
                    }
                }
                at += len;
            }
            None => at += 1,
        }
    }
    if seen == 0 {
        found.truncate(1);
    }
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
    let raw_frames = sprite::rs2_frames(rs2)?;
    let footprints = raw_frames
        .iter()
        .map(|f| if name.eq_ignore_ascii_case("dom") && !f.is_empty() { house_footprint(f) } else { None })
        .collect();
    let decks = raw_frames
        .iter()
        .map(|f| if name.eq_ignore_ascii_case("most") && !f.is_empty() { bridge_deck(f) } else { None })
        .collect();
    let frames = raw_frames
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
                // a bridge is its abutments then its deck, each with a shadow
                None => intact_sprites(frame, start, if name.eq_ignore_ascii_case("most") { 2 } else { 1 })
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
    Ok(ObjectSet { frames, footprints, decks })
}
