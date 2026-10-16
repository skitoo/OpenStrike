//! Single-player maps (`.ssm`, gzip, magic `SSSM`, version `0x10f`).
//!
//! Layout from the editor's map writer, `QED.EXE` `0x415fa0`. The sections up to the units are
//! parsed; see `docs/formats.md` for the rest.

use std::io::Read;

use flate2::read::GzDecoder;

use crate::reader::Reader;
use crate::{Error, Result};

/// Map pixels per cell.
pub const CELL: u32 = 32;

/// Ground types in the order of `desc_common/GROUNDTYPES`, indexed by bits 8-11 of [`Map::layer2`].
pub const GROUND_TYPES: [&str; 8] =
    ["grass", "lightgrass", "pashnya", "swamp", "darkground", "lightground", "water", "sand"];
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

/// A unit placed on the map (`QED.EXE` `0x4153e0` writes it, `Game_Dll.dll` `0x1003fe70` reads
/// it from `XCHNG\TOGAME\mis_mapunits`): 13 bytes, then `crew` records of 6 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapUnit {
    /// Index into [`Map::unit_types`] (byte 5).
    pub kind: u8,
    /// Owner (byte 12): the unit's id is taken from `player * 1024 + 1` on (`0x1003e370`).
    pub player: u8,
    /// Map pixels (bytes 7-10); a vehicle is moved to its cell's `(15, 15)` (`0x10040190`).
    pub x: i16,
    pub y: i16,
    /// Hull and turret heading, `& 31` (byte 11, `0x1002ce06`).
    pub heading: u8,
    /// Percentages of the type's hit points, ammunition (both kinds) and crew condition
    /// (bytes 1, 2, 3; `0x10040030`).
    pub health: u8,
    pub ammo: u8,
    pub crew_condition: u8,
    /// Byte 0, kept in `+0x5a` and passed with the player to `0x100681a0` (meaning not read).
    pub byte0: u8,
    /// Byte 4, kept in `+0x3f` (meaning not read).
    pub byte4: u8,
    /// Byte 6 records of 6 bytes (`+0x254` of the unit, crews of guns and the like; not read).
    pub crew: Vec<[u8; 6]>,
}

/// A player of the mission (`QED.EXE` `0x415b10`; `Game_Dll.dll` `0x1004f320` reads it from
/// `XCHNG\TOGAME\mis_players` in single player): a 0x25-byte record, then 4 records of 0x2c and 2
/// of 0x46 bytes (not read yet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapPlayer {
    /// The side's name, 32 bytes ("Spieler", "Gegner", "Verbündeter").
    pub name: String,
    /// Side (byte 0x20, the slot's `+0x24`): the single-player sides 0 and 2 are friends, 1 is
    /// hostile to both (`0x1004f320`).
    pub side: u8,
    /// Bytes 0x21-0x24 (slot `+0x21`, `+0x23`, `+0x26`; not read).
    pub extra: [u8; 4],
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
    /// Unit type names (`QED.EXE` `0x415140`, `XCHNG\TOGAME\mis_unitnames`): 16 bytes each.
    pub unit_types: Vec<String>,
    pub units: Vec<MapUnit>,
    pub players: Vec<MapPlayer>,
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
        cx < self.width
            && cy < self.height
            && (self.layer2[(cy * self.width + cx) as usize] >> ((tx & 1) + 2 * (ty & 1))) & 1 != 0
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
        .map(|_| Ok(Object { x: r.i16()?, y: r.i16()?, set: r.u8()?, frame: r.u8()?, extra: [r.u8()?, r.u8()?] }))
        .collect::<Result<_>>()?;
    let n_types = r.u32()? as usize;
    if n_types > 256 {
        return Err(Error::Invalid(format!("{n_types} unit types")));
    }
    let unit_types: Vec<String> = (0..n_types).map(|_| r.bytes(16).map(latin1)).collect::<Result<_>>()?;
    r.bytes(100 * 27)?; // QED.EXE 0x4151b0, not read yet
    // u32 size of what follows (the count and the records), u32 count, records
    let size = r.u32()? as usize;
    let start = r.pos();
    let count = r.u32()? as usize;
    let mut units = Vec::with_capacity(count);
    for _ in 0..count {
        let b = r.bytes(13)?;
        let crew = (0..b[6]).map(|_| r.bytes(6).map(|c| <[u8; 6]>::try_from(c).unwrap())).collect::<Result<_>>()?;
        let unit = MapUnit {
            byte0: b[0],
            health: b[1],
            ammo: b[2],
            crew_condition: b[3],
            byte4: b[4],
            kind: b[5],
            x: i16::from_le_bytes([b[7], b[8]]),
            y: i16::from_le_bytes([b[9], b[10]]),
            heading: b[11],
            player: b[12],
            crew,
        };
        if unit.kind as usize >= unit_types.len() {
            return Err(Error::Invalid(format!("unit type {} of {}", unit.kind, unit_types.len())));
        }
        units.push(unit);
    }
    if r.pos() - start != size {
        return Err(Error::Invalid(format!("unit section of {} bytes, {size} declared", r.pos() - start)));
    }
    let n_players = r.u32()? as usize;
    if n_players > 12 {
        return Err(Error::Invalid(format!("{n_players} players")));
    }
    let players = (0..n_players)
        .map(|_| {
            let b = r.bytes(0x25)?;
            let player =
                MapPlayer { name: latin1(&b[..0x20]), side: b[0x20], extra: [b[0x21], b[0x22], b[0x23], b[0x24]] };
            r.bytes(4 * 0x2c + 2 * 0x46)?;
            Ok(player)
        })
        .collect::<Result<_>>()?;
    Ok(Map {
        version,
        header,
        width,
        height,
        briefing,
        preview_size,
        preview,
        ground,
        layer2,
        object_sets,
        objects,
        unit_types,
        units,
        players,
    })
}
