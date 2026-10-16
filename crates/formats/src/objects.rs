//! Map object sets: `<set>.rs2` + `<set>.pl` in the climate archive (`dom` lives in `houses.rs2`).
//!
//! Every set is read the way its class in `Game_Dll.dll` loads it (first entry of the class vtable)
//! and drawn the way its drawing routine does (entry 12); see `docs/formats.md` and
//! `docs/original-code.md` for the addresses. The game draws objects on numbered layers, each one
//! sorted back to front: layers 0-5 lie on the ground, 6 holds the shadows, 7 the upright sprites.
//! The ground layer of each set comes from its `land/<SET>` description (`level`).

use crate::palette::Palette;
use crate::reader::Reader;
use crate::sprite::{self, Image};
use crate::{Error, Result};

/// The shadow layer and the upright layer, drawn after the ground layers 0-5.
pub const LAYER_SHADOW: u8 = 6;
pub const LAYER_UPRIGHT: u8 = 7;

/// Ground layer (`level` in `desc_*/land/<SET>`, the same in every climate) of a set.
pub fn ground_layer(set: &str) -> u8 {
    match set.to_ascii_lowercase().as_str() {
        "cliff" | "most" | "dom" => 0,
        "trop" => 1,
        "fre" | "stand" | "pont" => 2,
        "tree" | "vor" => 3,
        "lay" => 4,
        "zabor" => 5,
        _ => 0,
    }
}

#[derive(Debug, Clone)]
pub struct Part {
    /// Layer the game draws it on (0-5 ground, [`LAYER_SHADOW`], [`LAYER_UPRIGHT`]).
    pub layer: u8,
    pub image: Image,
    /// Added to the object's screen y when sorting upright parts (house halves, `0x100511d0`).
    pub depth: i32,
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

fn u32_at(frame: &[u8], at: usize) -> Option<usize> {
    frame.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
}

/// A sprite at `at`, or `None` for an empty (0 x 0) one or one that does not decode.
fn sprite_at(frame: &[u8], at: usize, palette: &Palette) -> Option<Image> {
    let image = sprite::decode(frame.get(at..)?, palette).ok()?;
    (image.width > 0 && image.height > 0).then_some(image)
}

/// House states, read as the loader does (`Game_Dll.dll` `0x10050980`): after the 512-byte
/// palette, a u16, a u32 and the 14x14 footprint, two i32 depth offsets (left and right half,
/// `+0x2da`, `+0x2de` of the house type), 3 bytes, a u32 count `n` and `9 n` bytes (entrances), a
/// byte and three u16, then for each of the 4 states 4 sprites, each a u32 length and that many
/// bytes (0 = none): left half, right half, ground, shadow. The drawing (`0x100511d0`) puts the
/// halves on layer 7 sorted by the house's screen y plus their depth offset, the ground on the
/// set's ground layer and the shadow on layer 6.
fn house_states(frame: &[u8], palette: &Palette) -> Option<Vec<Vec<Part>>> {
    let mut at = 518 + FOOTPRINT_SIZE * FOOTPRINT_SIZE;
    let i32_at = |at: usize| u32_at(frame, at).map(|v| v as u32 as i32);
    let depths = [i32_at(at)?, i32_at(at + 4)?];
    at += 8 + 3;
    let n = u32_at(frame, at)?;
    at += 4 + 9 * n + 1 + 6;
    let mut states = Vec::with_capacity(4);
    for _ in 0..4 {
        let mut parts = Vec::new();
        for (k, layer) in [LAYER_UPRIGHT, LAYER_UPRIGHT, ground_layer("dom"), LAYER_SHADOW].into_iter().enumerate() {
            let len = u32_at(frame, at)?;
            at += 4;
            let sprite = frame.get(at..at + len)?;
            at += len;
            if len > 0
                && let Some(image) = sprite_at(sprite, 0, palette)
            {
                parts.push(Part { layer, image, depth: if k < 2 { depths[k] } else { 0 } });
            }
        }
        states.push(parts);
    }
    // the loader reads exactly up to the next frame
    (at == frame.len()).then_some(states)
}

/// Bridge states (loader `0x10052040`): after the 512-byte palette, a u32 and the 22x22 grid, at
/// byte 1000, a table of 10 u32 offsets relative to byte 1000: abutments, their shadow, then the
/// deck and its shadow for states 0 to 3 (an empty 0 x 0 sprite is none). The drawing
/// (`0x10052450`) puts the abutments and the deck of the state on the ground layer, their shadows on
/// layer 6. Four states match the four state bits of the grid (`BRIDGE_OPEN << state`).
fn bridge_states(frame: &[u8], palette: &Palette) -> Option<Vec<Vec<Part>>> {
    let table = Reader::at(frame, 1000).ok()?.u32_vec(10).ok()?;
    let part = |i: usize, layer: u8| {
        sprite_at(frame, table[i] as usize + 1000, palette).map(|image| Part { layer, image, depth: 0 })
    };
    let ground = ground_layer("most");
    let abutments = [part(0, ground), part(1, LAYER_SHADOW)];
    Some(
        (0..4)
            .map(|s| {
                abutments
                    .iter()
                    .cloned()
                    .chain([part(2 + 2 * s, ground), part(3 + 2 * s, LAYER_SHADOW)])
                    .flatten()
                    .collect()
            })
            .collect(),
    )
}

/// Archive file name (without extension) of a set name used in maps.
pub fn file_stem(set: &str) -> &str {
    if set.eq_ignore_ascii_case("dom") { "houses" } else { set }
}

/// `tree` (loader `0x100557e0`, drawing `0x10055d80`) and `stand` (`0x10054950`, `0x100551a0`)
/// frames: a table of 6 u32 sprite offsets (0 = none), at byte 5 for trees and 205 for `stand`
/// (after a u32 palette index, a byte? and the 14x14 footprint at byte 9): upright intact and
/// destroyed, shadow intact and destroyed, ground intact and destroyed.
fn slotted_states(frame: &[u8], table: usize, ground: u8, palette: &Palette) -> Vec<Vec<Part>> {
    (0..2)
        .map(|state| {
            [(state, LAYER_UPRIGHT), (2 + state, LAYER_SHADOW), (4 + state, ground)]
                .into_iter()
                .filter_map(|(slot, layer)| {
                    let at = u32_at(frame, table + 4 * slot).filter(|&a| a != 0)?;
                    Some(Part { layer, image: sprite_at(frame, at, palette)?, depth: 0 })
                })
                .collect()
        })
        .collect()
}

/// Drawing state of an object from its map state byte (`Object::extra[0]`), as each class encodes
/// it: `stand` is destroyed when the byte is not zero (`Game_Dll.dll` `0x10054b60`), trees and
/// `lay` use its low bit (`0x10053c30`), houses (`0x100500c0`) and bridges use it as is (0 to 3);
/// `cliff`, `fre` and `trop` have no state (`0x10053680`).
pub fn map_state(set: &str, extra0: u8) -> usize {
    match set.to_ascii_lowercase().as_str() {
        "stand" => (extra0 != 0) as usize,
        "tree" | "lay" => (extra0 & 1) as usize,
        "dom" | "most" => (extra0 & 3) as usize,
        _ => 0,
    }
}

/// Where the game puts an object of `set` stored at `(x, y)` in a map of `width` x `height` cells
/// (the class's placement routine), or `None` when it drops it: houses, trees, `stand` and fences
/// must be at least 4 cells from the edges, the others only inside the map.
///
/// - `cliff`, `dom`: the corner of the cell (`0x10052f50`, `0x10050d40`);
/// - `tree`, `stand`, `lay`: the middle of the 16 px quarter (`0x100559d0`, `0x10053cd0`);
/// - `fre`, `trop`, `vor`: the middle of the 8 px square (`0x100566d0`);
/// - `zabor`: the middle of the cell (`0x10057070`);
/// - `most`: as stored.
pub fn placement(set: &str, x: i16, y: i16, width: u32, height: u32) -> Option<(i32, i32)> {
    let (x, y) = (x as i32, y as i32);
    let set = set.to_ascii_lowercase();
    let (px, py) = match set.as_str() {
        "cliff" | "dom" => (x & !31, y & !31),
        "tree" | "stand" | "lay" => ((x & !15) + 8, (y & !15) + 8),
        "fre" | "trop" | "vor" => ((x & !7) + 4, (y & !7) + 4),
        "zabor" => ((x & !31) + 16, (y & !31) + 16),
        _ => (x, y),
    };
    let (cx, cy, w, h) = (px >> 5, py >> 5, width as i32, height as i32);
    let inside = match set.as_str() {
        "dom" | "tree" | "stand" | "zabor" => cx >= 4 && cy >= 4 && cx < w - 4 && cy < h - 4,
        _ => px >= 0 && py >= 0 && cx < w && cy < h,
    };
    inside.then_some((px, py))
}

/// Fence (`zabor`) frame from its extra bytes. The game repacks each fence into a 5-byte cell
/// record (`Game_Dll.dll` `0x10056e80`: position to 8 px, `extra[0] & 3`, `extra[1]`, and
/// `(frame % 40) / 8` or `frame & 7`) and rebuilds the frame from it (`0x10056f60`):
/// `extra[0]` is the fence kind, a block of 40 frames; `extra[1]` bits 1-3 the piece and bit 0 an
/// alternative look. Pieces 0-3: `kind * 40 + variant * 8 + look * 4 + piece`; piece 4 (a post):
/// `kind * 40 + (3 + look) * 8 + (frame & 7)`. All 190,284 fences of the single-player maps give
/// back their frame.
pub fn fence_frame(frame: u8, extra: [u8; 2]) -> u8 {
    let (kind, look, piece) = ((extra[0] & 3) as u32, (extra[1] & 1) as u32, ((extra[1] >> 1) & 7) as u32);
    if piece == 4 {
        (((kind * 5 + look + 3) << 3) + (frame & 7) as u32) as u8
    } else {
        let variant = ((frame % 40) / 8) as u32;
        ((((variant + kind * 5) * 2 + look) * 4) + piece) as u8
    }
}

/// Whether a fence lies flat: the alternative look (`extra[1]` bit 0) is drawn on the ground
/// layer instead of upright (`0x10057408`).
pub fn fence_lies_flat(extra: [u8; 2]) -> bool {
    extra[1] & 1 != 0
}

/// Decode every frame of a set as its class loads it. `palettes` (the set's `.pl`) is required
/// unless the set embeds its own (`dom`, `most`); `stand` frames pick one by their first u32, the
/// others use the first.
pub fn load_set(name: &str, rs2: &[u8], palettes: &[Palette]) -> Result<ObjectSet> {
    let set = name.to_ascii_lowercase();
    let own_palette = matches!(set.as_str(), "dom" | "most");
    if !own_palette && palettes.is_empty() {
        return Err(Error::Invalid(format!("object set {name} needs a palette")));
    }
    let raw_frames = sprite::rs2_frames(rs2)?;
    let footprints =
        raw_frames.iter().map(|f| if set == "dom" && !f.is_empty() { house_footprint(f) } else { None }).collect();
    let bridges =
        raw_frames.iter().map(|f| if set == "most" && !f.is_empty() { bridge_grid(f) } else { None }).collect();
    let ground = ground_layer(&set);
    let frames = raw_frames
        .into_iter()
        .map(|frame| {
            if frame.is_empty() {
                return ObjectFrame::default();
            }
            let palette = if own_palette {
                match Palette::parse(frame) {
                    Ok(p) => p,
                    Err(_) => return ObjectFrame::default(),
                }
            } else if set == "stand" {
                let select = u32_at(frame, 0).unwrap_or(0);
                palettes.get(select).unwrap_or(&palettes[0]).clone()
            } else {
                palettes[0].clone()
            };
            let one = |layer: u8, at: usize| -> Vec<Part> {
                sprite_at(frame, at, &palette).map(|image| Part { layer, image, depth: 0 }).into_iter().collect()
            };
            let states: Vec<Vec<Part>> = match set.as_str() {
                "dom" => house_states(frame, &palette).unwrap_or_default(),
                "most" => bridge_states(frame, &palette).unwrap_or_default(),
                "tree" => slotted_states(frame, 5, ground, &palette),
                "stand" => slotted_states(frame, 205, ground, &palette),
                // loader 0x10052dc0, drawing 0x10053010: two u32 offsets in the frame (0 = none),
                // drawn on the layers `level1` and `level2` (both 0)
                "cliff" => vec![
                    [0, 1]
                        .into_iter()
                        .filter_map(|i| u32_at(frame, 4 * i).filter(|&a| a != 0))
                        .flat_map(|at| one(ground, at))
                        .collect(),
                ],
                // loader 0x100539f0, drawing 0x10053fc0: a byte saying whether a second state
                // follows, then u32 length + sprite, and again for the second state
                "lay" => {
                    let len0 = u32_at(frame, 1).unwrap_or(0);
                    let mut states = vec![one(ground, 5)];
                    if frame[0] != 0 && len0 > 0 {
                        states.push(one(ground, 5 + len0 + 4));
                    }
                    states
                }
                // loader 0x10056dc0, drawing 0x10057310: u32 length + upright sprite, then the shadow
                "zabor" => {
                    let len = u32_at(frame, 0).unwrap_or(0);
                    vec![one(LAYER_UPRIGHT, 4).into_iter().chain(one(LAYER_SHADOW, 4 + len)).collect()]
                }
                // `fre`, `trop` (loader 0x10053850), `vor` (0x10056510): the frame is the sprite
                _ => vec![one(ground, 0)],
            };
            let states = if states.first().is_some_and(|s| !s.is_empty()) { states } else { Vec::new() };
            ObjectFrame { states }
        })
        .collect();
    Ok(ObjectSet { frames, footprints, bridges })
}
