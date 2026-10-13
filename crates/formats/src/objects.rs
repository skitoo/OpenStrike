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
use crate::reader::Reader;
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
/// Grid cell under the object position: the `stand` routines compute the tile as `(x >> 4) + gx - 6`
/// (see [`FOOTPRINT_SOLID`]); for houses the data agrees (75 of the 113 summer houses, and the stored
/// passability).
pub const FOOTPRINT_ORIGIN: i32 = 6;
pub type Footprint = [[u8; FOOTPRINT_SIZE]; FOOTPRINT_SIZE];

/// Footprint cells that block: bit 0x01, as in the `stand` footprint routines of the original
/// (`Game_Dll.dll` `0x10054cda`, `0x10054f02`, `0x10055694`); for houses, every such tile is blocked in
/// the passability the editor stored (see [`crate::map::Map::blocked`]), but the house routine itself
/// has not been read yet.
pub const FOOTPRINT_SOLID: u8 = 0x01;

/// Side of a bridge grid (`most`), in tiles of 16 map pixels.
pub const BRIDGE_SIZE: usize = 22;
/// Bridge grid value bit: the tile belongs to the bridge.
pub const BRIDGE_PART: u8 = 0x01;
/// Bridge grid value bit for state `s` (0 = intact): `BRIDGE_OPEN << s` set means the tile is open
/// to vehicles in that state, clear means blocked.
pub const BRIDGE_OPEN: u8 = 0x10;

/// The 16 px tile of a bridge grid tile, as the original computes it (`Game_Dll.dll` `0x10051708`):
/// cell `(x >> 5) + (gx >> 1) - 5`, quarter `gx & 1` (same for y).
pub fn bridge_tile(x: i16, y: i16, gx: i32, gy: i32) -> (i32, i32) {
    let cell = |p: i16, g: i32| ((p as i32) >> 5) + (g >> 1) - 5;
    (2 * cell(x, gx) + (gx & 1), 2 * cell(y, gy) + (gy & 1))
}

/// The parts of one object frame in each state it has; state 0 is intact. Objects store their state
/// in the map (`Object::extra[0]`). Only bridges have their other states decoded so far.
#[derive(Debug, Clone, Default)]
pub struct ObjectFrame {
    pub states: Vec<Vec<Part>>,
}

impl ObjectFrame {
    /// Parts of `state`, falling back to the intact state when that state is not decoded.
    pub fn parts(&self, state: usize) -> &[Part] {
        self.states.get(state).or(self.states.first()).map_or(&[], Vec::as_slice)
    }

    /// The state actually drawn for a requested one.
    pub fn resolve(&self, state: usize) -> usize {
        if state < self.states.len() { state } else { 0 }
    }
}

/// Every frame of a set; empty frames have no states.
#[derive(Debug, Clone, Default)]
pub struct ObjectSet {
    pub frames: Vec<ObjectFrame>,
    /// Building footprints (`dom` only), per frame.
    pub footprints: Vec<Option<Footprint>>,
    /// Bridge grids (`most` only), per frame, row by row.
    pub bridges: Vec<Option<Vec<u8>>>,
}

/// A bridge frame: 512-byte palette, two u16, then the 22x22 grid ending at byte 1000, where a table
/// of sprite offsets (relative to byte 1000) starts. With the grid at 516, 98.9 % of the bridge tiles
/// of 60 maps match the passability the editor stored (76 % at 518).
fn bridge_grid(frame: &[u8]) -> Option<Vec<u8>> {
    frame.get(516..516 + BRIDGE_SIZE * BRIDGE_SIZE).map(<[u8]>::to_vec)
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

/// Bridge states: after the grid, at byte 1000, a table of 11 u32 offsets relative to byte 1000:
/// abutments, their shadow, then the deck and its shadow for states 0 to 3, then the end of the frame.
/// Four states match the four state bits of the grid (`BRIDGE_OPEN << state`).
fn bridge_states(frame: &[u8], palette: &Palette) -> Option<Vec<Vec<Part>>> {
    let table = Reader::at(frame, 1000).ok()?.u32_vec(11).ok()?;
    let at = |i: usize| table[i] as usize + 1000;
    // every entry is a sprite that parses exactly, or an empty 0x0 one (no shadow for that state,
    // bridge frames 24 and 26); anything else means the frame is not a 4-state bridge
    let decode = |i: usize, layer: Layer| -> Option<Option<Part>> {
        let header = Reader::at(frame, at(i)).ok().and_then(|mut r| Some((r.i16().ok()?, r.i16().ok()?, r.u16().ok()?, r.u16().ok()?)));
        if let Some((_, _, 0, _) | (_, _, _, 0)) = header {
            return Some(None);
        }
        sprite::frame_len(frame, at(i))?;
        Some(Some(Part { layer, image: sprite::decode(&frame[at(i)..], palette).ok()? }))
    };
    let abutments = [decode(0, Layer::Flat)?, decode(1, Layer::Shadow)?];
    (0..4)
        .map(|s| {
            let deck = [decode(2 + 2 * s, Layer::Flat)?, decode(3 + 2 * s, Layer::Shadow)?];
            Some(abutments.iter().chain(&deck).flatten().cloned().collect())
        })
        .collect()
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

/// Sprites of `state` (0 or 1) from the slot table: (offset, layer) for slots `state`, `2 + state` and
/// `4 + state`, as the original draws them (`Game_Dll.dll` `0x100551a0` for `stand`, `0x10055e01` for trees).
fn slotted_sprites(frame: &[u8], table: usize, state: usize) -> Vec<(usize, Layer)> {
    [(state, Layer::Upright), (2 + state, Layer::Shadow), (4 + state, Layer::Flat)]
        .into_iter()
        .filter_map(|(slot, layer)| {
            let at = crate::reader::u32_at(frame, table + 4 * slot).ok()? as usize;
            (at != 0 && sprite::frame_len(frame, at).is_some()).then_some((at, layer))
        })
        .collect()
}

/// Drawing state of an object from its map state byte (`Object::extra[0]`), per set, as the original
/// builds its objects: `stand` is destroyed when the byte is not zero (`Game_Dll.dll` `0x10054b61`),
/// trees use its low bit (`0x100560e5`), houses (`0x100500c0`) and bridges use it as is (0 to 3).
/// Other sets are always drawn intact.
pub fn map_state(set: &str, extra0: u8) -> usize {
    match set.to_ascii_lowercase().as_str() {
        "stand" => (extra0 != 0) as usize,
        "tree" => (extra0 & 1) as usize,
        "dom" | "most" => (extra0 & 3) as usize,
        _ => 0,
    }
}

/// The sprites of a frame from `start`, cut into groups that each end with a shadow; sprites after
/// the last shadow form a last group.
fn sprite_groups(frame: &[u8], start: usize) -> Vec<Vec<(usize, u8)>> {
    let mut groups = vec![Vec::new()];
    let mut at = start;
    while at + 9 <= frame.len() {
        match sprite::frame_len(frame, at) {
            Some(len) => {
                let kind = frame[at + 8];
                groups.last_mut().unwrap().push((at, kind));
                if kind == KIND_SHADOW {
                    groups.push(Vec::new());
                }
                at += len;
            }
            None => at += 1,
        }
    }
    groups.retain(|g| !g.is_empty());
    groups
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
    let bridges = raw_frames
        .iter()
        .map(|f| if name.eq_ignore_ascii_case("most") && !f.is_empty() { bridge_grid(f) } else { None })
        .collect();
    let frames = raw_frames
        .into_iter()
        .map(|frame| {
            let (pal, start) = if own_palette {
                match Palette::parse(frame) {
                    Ok(p) => (p, 512),
                    Err(_) => return ObjectFrame::default(),
                }
            } else {
                let select = match palettes.len() {
                    1 => 0,
                    _ => frame.get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()) as usize),
                };
                (palettes.get(select).unwrap_or(&palettes[0]).clone(), 0)
            };
            if name.eq_ignore_ascii_case("most")
                && let Some(states) = bridge_states(frame, &pal)
            {
                return ObjectFrame { states };
            }
            let decode = |sprites: Vec<(usize, Layer)>| -> Vec<Part> {
                sprites
                    .into_iter()
                    .filter_map(|(at, layer)| Some(Part { layer, image: sprite::decode(&frame[at..], &pal).ok()? }))
                    .collect()
            };
            let layered = |group: Vec<(usize, u8)>| -> Vec<(usize, Layer)> {
                group
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
                    .collect()
            };
            let states: Vec<Vec<Part>> = if let Some(table) = slot_table(name) {
                (0..2).map(|s| decode(slotted_sprites(frame, table, s))).collect()
            } else {
                let mut groups = sprite_groups(frame, start);
                if name.eq_ignore_ascii_case("dom") {
                    // a house: one group per state, 4 states (left half, right half, ground, shadow)
                    groups.into_iter().take(4).map(|g| decode(layered(g))).collect()
                } else if name.eq_ignore_ascii_case("most") {
                    // a bridge without a readable state table: abutments then deck, each with a shadow
                    let both: Vec<_> = groups.into_iter().take(2).flatten().collect();
                    vec![decode(layered(both))]
                } else {
                    let first = groups.drain(..).next().unwrap_or_default();
                    let has_shadow = first.last().is_some_and(|&(_, k)| k == KIND_SHADOW);
                    vec![decode(layered(if has_shadow { first } else { first.into_iter().take(1).collect() }))]
                }
            };
            let states = if states.first().is_some_and(|s| !s.is_empty()) { states } else { Vec::new() };
            ObjectFrame { states }
        })
        .collect();
    Ok(ObjectSet { frames, footprints, bridges })
}
