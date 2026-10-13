//! Loading game data into plain values, before anything touches the GPU.

use std::path::Path;

use openstrike_formats::hot;
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

/// Sprites of a turreted vehicle, each strip holding the 32 headings.
pub struct VehicleSprites {
    pub hull: Vec<Image>,
    pub hull_shadow: Vec<Image>,
    pub turret: Vec<Image>,
    pub turret_shadow: Vec<Image>,
    /// Turret mount on the hull, per hull frame.
    pub mounts: Vec<(i32, i32)>,
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
    // Observed layout (t-26.pck): hull and damage variants, hull shadows, turret, turret shadows.
    let shadow_runs: Vec<usize> = (0..frames.len())
        .filter(|&i| {
            sprite::frame_kind(frames[i]) == Some(KIND_SHADOW)
                && (i == 0 || sprite::frame_kind(frames[i - 1]) != Some(KIND_SHADOW))
        })
        .collect();
    let [hull_shadow, turret_shadow] = shadow_runs[..] else {
        return Err(format!("{name}: expected 2 shadow runs, found {}", shadow_runs.len()).into());
    };
    let turret = hull_shadow + 32;
    let decode = |start: usize| -> Result<Vec<Image>, Error> {
        let strip = frames.get(start..start + 32).ok_or("sprite strip out of range")?;
        strip.iter().map(|f| Ok(sprite::decode(f, &palette)?)).collect()
    };
    let mut mounts = hot::parse(&vfs.require(&format!("{name}.hot"))?)?;
    mounts.truncate(32);
    Ok(VehicleSprites {
        hull: decode(0)?,
        hull_shadow: decode(hull_shadow)?,
        turret: decode(turret)?,
        turret_shadow: decode(turret_shadow)?,
        mounts,
    })
}

pub fn load(data_dir: &Path, map_path: &Path, climate: &str, tank: &str) -> Result<Assets, Error> {
    let vfs = mount(data_dir, climate)?;
    let map = map::parse(&std::fs::read(map_path)?)?;
    let rhombs = vfs.require("mappic.rs2")?;
    let palettes = Palette::parse_many(&vfs.require("rhomb.pl")?)?;
    let ground = ground_chunks(&map, &rhombs, &palettes);
    Ok(Assets { map, ground, tank: vehicle(&vfs, tank)? })
}
