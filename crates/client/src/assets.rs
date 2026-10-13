//! Loading game data into plain values, before anything touches the GPU.

use std::path::Path;

use openstrike_formats::map::{self, Map};
use openstrike_formats::palette::Palette;
use openstrike_formats::rhomb::{Rect, rasterize_ground};
use openstrike_formats::sprite::{self, Image, KIND_SHADOW};
use openstrike_formats::sue::{Archive, Vfs};

pub type Error = Box<dyn std::error::Error>;

/// Size of the ground tiles uploaded as textures, in map pixels.
pub const CHUNK: i32 = 512;

pub struct Chunk {
    pub rect: Rect,
    pub rgba: Vec<u8>,
}

/// Hull frames of a vehicle (32 directions) and their shadows.
pub struct VehicleSprites {
    pub hull: Vec<Image>,
    pub shadow: Vec<Image>,
}

pub struct Assets {
    pub map: Map,
    pub ground: Vec<Chunk>,
    pub tank: VehicleSprites,
}

pub const CLIMATES: [&str; 4] = ["beach", "desert", "summer", "winter"];

/// Climate an archive belongs to (`summer01`, `desc_summer`, `summer_edit` -> summer), if any.
fn archive_climate(stem: &str) -> Option<&'static str> {
    let stem = stem.to_ascii_lowercase();
    let stem = stem.strip_prefix("desc_").unwrap_or(&stem);
    CLIMATES.into_iter().find(|c| stem.starts_with(c))
}

/// Mount the shared archives, then the ones of `climate`: climate archives reuse the same names
/// (every one has a `mappic.rs2`), so only one climate can be mounted at a time.
pub fn mount(data_dir: &Path, climate: &str) -> Result<Vfs, Error> {
    let mut paths: Vec<_> = std::fs::read_dir(data_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sue")))
        .collect();
    paths.sort_by_key(|p| {
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        (archive_climate(&stem).is_some(), stem)
    });
    let mut vfs = Vfs::default();
    for p in paths {
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if archive_climate(&stem).is_none_or(|c| c == climate) {
            vfs.mount(Archive::parse(std::fs::read(&p)?)?);
        }
    }
    Ok(vfs)
}

pub fn ground_chunks(map: &Map, rhombs: &[u8], palettes: &[Palette]) -> Vec<Chunk> {
    let (w, h) = map.pixel_size();
    let mut chunks = Vec::new();
    for y in (0..h as i32).step_by(CHUNK as usize) {
        for x in (0..w as i32).step_by(CHUNK as usize) {
            let rect = Rect { x, y, w: CHUNK.min(w as i32 - x), h: CHUNK.min(h as i32 - y) };
            chunks.push(Chunk { rect, rgba: rasterize_ground(map, rhombs, palettes, rect) });
        }
    }
    chunks
}

pub fn vehicle(vfs: &Vfs, name: &str) -> Result<VehicleSprites, Error> {
    let pck = vfs.require(&format!("{name}.pck"))?;
    let palette = Palette::parse(&vfs.require(&format!("{name}.col"))?)?;
    let frames = sprite::pck_frames(&pck)?;
    let first_shadow = frames
        .iter()
        .position(|f| sprite::frame_kind(f) == Some(KIND_SHADOW))
        .ok_or("vehicle without shadow frames")?;
    let decode = |range: std::ops::Range<usize>| -> Result<Vec<Image>, Error> {
        frames[range].iter().map(|f| Ok(sprite::decode(f, &palette)?)).collect()
    };
    Ok(VehicleSprites { hull: decode(0..32)?, shadow: decode(first_shadow..first_shadow + 32)? })
}

pub fn load(data_dir: &Path, map_path: &Path, climate: &str, tank: &str) -> Result<Assets, Error> {
    let vfs = mount(data_dir, climate)?;
    let map = map::parse(&std::fs::read(map_path)?)?;
    let rhombs = vfs.require("mappic.rs2")?;
    let palettes = Palette::parse_many(&vfs.require("rhomb.pl")?)?;
    let ground = ground_chunks(&map, &rhombs, &palettes);
    Ok(Assets { map, ground, tank: vehicle(&vfs, tank)? })
}
