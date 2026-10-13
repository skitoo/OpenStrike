//! Single-player maps (`.ssm`, gzip, magic `SSSM`, version `0x10f`).
//!
//! Layout from the editor's map writer, `QED.EXE` `0x415fa0`. Only the sections up to the object
//! list are parsed; see `docs/formats.md` for the rest.

use std::io::Read;

use flate2::read::GzDecoder;

use crate::reader::Reader;
use crate::{Error, Result};

/// Map pixels per cell.
pub const CELL: u32 = 32;

/// Ground types in the order of `desc_common/GROUNDTYPES`, indexed by bits 8-11 of [`Map::layer2`].
pub const GROUND_TYPES: [&str; 8] = ["grass", "lightgrass", "pashnya", "swamp", "darkground", "lightground", "water", "sand"];
pub const GROUND_WATER: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Object {
    /// Position in map pixels.
    pub x: i16,
    pub y: i16,
    /// Index into [`Map::object_sets`].
    pub set: u8,
    pub frame: u8,
    pub extra: [u8; 2],
}

#[derive(Debug, Clone)]
pub struct Map {
    pub version: u32,
    pub header: [u32; 5],
    /// Size in cells.
    pub width: u32,
    pub height: u32,
    pub briefing: String,
    /// RGB565 minimap, `preview_size` pixels.
    pub preview_size: (u32, u32),
    pub preview: Vec<u16>,
    /// Per cell: `palette_select << 11 | rhomb_index`.
    pub ground: Vec<u16>,
    /// Per cell. Bits 0-3: blocked quarters (see [`Map::blocked`]); bits 8-11: ground type
    /// (see [`GROUND_TYPES`]); the rest is unknown.
    pub layer2: Vec<u32>,
    pub object_sets: Vec<String>,
    pub objects: Vec<Object>,
}

impl Map {
    pub fn rhomb(&self, x: u32, y: u32) -> (u16, u8) {
        let v = self.ground[(y * self.width + x) as usize];
        (v & 0x7ff, (v >> 11) as u8)
    }

    /// Whether a 16 px tile (a quarter of a cell) is blocked to vehicles. The map stores the game's
    /// own passability: the low 4 bits of each cell, bit `(tx & 1) + 2 * (ty & 1)`, 1 = blocked.
    /// Water, houses and intact bridges are already baked in by the editor. The game updates these
    /// bits when a bridge changes state (`Game_Dll.dll` `0x100516e7`) or a house is destroyed
    /// (`0x10054cda`); checked against the data: every solid house tile is set, and 98.9 % of the
    /// bridge tiles of 60 maps match the bridge rule.
    pub fn blocked(&self, tx: u32, ty: u32) -> bool {
        let (cx, cy) = (tx / 2, ty / 2);
        cx < self.width && cy < self.height && (self.layer2[(cy * self.width + cx) as usize] >> ((tx & 1) + 2 * (ty & 1))) & 1 != 0
    }

    /// Ground type index of a cell (see [`GROUND_TYPES`]).
    pub fn ground_type(&self, x: u32, y: u32) -> u8 {
        ((self.layer2[(y * self.width + x) as usize] >> 8) & 0xf) as u8
    }

    /// Climate of the map from `header[4]`: 0 summer, 1 winter, 2 beach, 3 desert, matching the
    /// archive schemes 1-4 of `sudtest.ini` (observed on the 164 single-player maps: winter and
    /// desert maps carry 1 and 3; where the game reads it is not checked yet).
    pub fn climate(&self) -> &'static str {
        match self.header[4] {
            1 => "winter",
            2 => "beach",
            3 => "desert",
            _ => "summer",
        }
    }

    pub fn pixel_size(&self) -> (u32, u32) {
        (self.width * CELL, self.height * CELL)
    }
}

pub fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    GzDecoder::new(data).read_to_end(&mut out)?;
    Ok(out)
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect()
}

/// Parse a `.ssm` file as stored on disk (gzip-compressed).
pub fn parse(file: &[u8]) -> Result<Map> {
    parse_raw(&gunzip(file)?)
}

pub fn parse_raw(data: &[u8]) -> Result<Map> {
    let mut r = Reader::new(data);
    match r.bytes(4)? {
        b"SSSM" => {}
        b"SSMM" => return Err(Error::Unsupported("multiplayer map (SSMM)".into())),
        b"CAMP" => return Err(Error::Unsupported("campaign (CAMP)".into())),
        _ => return Err(Error::BadMagic { expected: "SSSM" }),
    }
    let version = r.u32()?;
    r.bytes(16)?; // GUID
    let mut header = [0u32; 5];
    for h in &mut header {
        *h = r.u32()?;
    }
    let (width, height) = (r.u32()?, r.u32()?);
    if width == 0 || height == 0 || width > 1024 || height > 1024 {
        return Err(Error::Invalid(format!("map size {width}x{height}")));
    }
    let cells = (width * height) as usize;
    let briefing_len = r.u32()? as usize;
    let briefing = latin1(r.bytes(briefing_len)?);
    let preview_size = if width <= 128 && height <= 128 { (width, height) } else { (width / 2, height / 2) };
    let preview = (0..preview_size.0 * preview_size.1).map(|_| r.u16()).collect::<Result<_>>()?;
    let ground = (0..cells).map(|_| r.u16()).collect::<Result<_>>()?;
    let layer2 = r.u32_vec(cells)?;
    let n_sets = r.u32()? as usize;
    if n_sets > 64 {
        return Err(Error::Invalid(format!("{n_sets} object sets")));
    }
    let object_sets = (0..n_sets).map(|_| r.bytes(16).map(latin1)).collect::<Result<_>>()?;
    let n_objects = r.u32()? as usize;
    let objects = (0..n_objects)
        .map(|_| {
            Ok(Object { x: r.i16()?, y: r.i16()?, set: r.u8()?, frame: r.u8()?, extra: [r.u8()?, r.u8()?] })
        })
        .collect::<Result<_>>()?;
    Ok(Map { version, header, width, height, briefing, preview_size, preview, ground, layer2, object_sets, objects })
}
