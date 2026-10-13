//! Loading game data into plain values, before anything touches the GPU.

use std::path::Path;

use openstrike_formats::{curves, desc, hot};
use openstrike_formats::iso::Projection;
use openstrike_formats::map::{self, Map};
use openstrike_formats::objects::{self, ObjectSet};
use openstrike_formats::palette::Palette;
use openstrike_formats::sprite::{self, Image, KIND_SHADOW};
use openstrike_formats::sue::{Archive, Vfs};
use openstrike_sim::UnitType;

pub type Error = Box<dyn std::error::Error>;

/// Sprites of a turreted vehicle, each strip holding the 32 headings.
pub struct VehicleSprites {
    pub hull: Vec<Image>,
    pub hull_shadow: Vec<Image>,
    pub turret: Vec<Image>,
    pub turret_shadow: Vec<Image>,
    /// Turret mount on the hull, per hull frame.
    pub mounts: Vec<(i32, i32)>,
}

/// A vehicle type: its description and its sprites.
pub struct Vehicle {
    /// Description name (`units/<name>` in `desc_common`).
    pub name: String,
    pub unit_type: UnitType,
    pub sprites: VehicleSprites,
}

pub struct Assets {
    pub map: Map,
    pub projection: Projection,
    pub rhombs: Vec<u8>,
    pub rhomb_palettes: Vec<Palette>,
    /// One per entry of `map.object_sets`; empty when the set could not be loaded.
    pub object_sets: Vec<ObjectSet>,
    /// The vehicles that could be loaded, in the order asked.
    pub vehicles: Vec<Vehicle>,
    /// `CURVENEW` points per `next * 9 + previous`.
    pub curves: Vec<Vec<(i16, i16)>>,
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
    let stem = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    paths.sort_by_key(|p| (archive_climate(&stem(p)).is_some(), stem(p)));
    let mut vfs = Vfs::default();
    for p in paths {
        if archive_climate(&stem(&p)).is_none_or(|c| c == climate) {
            vfs.mount(Archive::parse(std::fs::read(&p)?)?);
        }
    }
    Ok(vfs)
}

/// Movement parameters and sprite file of a unit from its description.
pub fn unit_type(vfs: &Vfs, name: &str) -> Result<(UnitType, String), Error> {
    let d = desc::load(&format!("units/{name}"), &mut |n| vfs.read(n).ok().flatten())?;
    let missing = |key: &str| format!("{name}: no {key} in the description");
    let speed = (d.float("MoveSpeed", 0).ok_or_else(|| missing("MoveSpeed"))?, d.float("MoveSpeed", 1).ok_or_else(|| missing("MoveSpeed"))?);
    let turn = (d.int("TurnDelay", 0).ok_or_else(|| missing("TurnDelay"))?, d.int("TurnDelay", 1).ok_or_else(|| missing("TurnDelay"))?);
    let mut t = UnitType::new(name, speed, turn, d.int("GunTurnDelay", 0).unwrap_or(0));
    t.ignore_mines = d.int("IgnoreMines", 0).unwrap_or(0) != 0;
    t.can_move_backward = d.int("CanMoveBackward", 0).unwrap_or(0) != 0;
    let file = d.get("File").and_then(|v| v.first()).ok_or_else(|| missing("File"))?.to_ascii_lowercase();
    Ok((t, file))
}

pub fn vehicle(vfs: &Vfs, name: &str) -> Result<Vehicle, Error> {
    let (unit_type, file) = unit_type(vfs, name)?;
    Ok(Vehicle { name: name.to_string(), unit_type, sprites: vehicle_sprites(vfs, &file)? })
}

pub fn vehicle_sprites(vfs: &Vfs, name: &str) -> Result<VehicleSprites, Error> {
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
    let decode = |start: usize| -> Result<Vec<Image>, Error> {
        let strip = frames.get(start..start + 32).ok_or("sprite strip out of range")?;
        strip.iter().map(|f| Ok(sprite::decode(f, &palette)?)).collect()
    };
    let mut mounts = hot::parse(&vfs.require(&format!("{name}.hot"))?)?;
    mounts.truncate(32);
    Ok(VehicleSprites {
        hull: decode(0)?,
        hull_shadow: decode(hull_shadow)?,
        turret: decode(hull_shadow + 32)?,
        turret_shadow: decode(turret_shadow)?,
        mounts,
    })
}

fn object_set(vfs: &Vfs, name: &str) -> Result<ObjectSet, Error> {
    let Some(rs2) = vfs.read(&format!("{}.rs2", objects::file_stem(name)))? else {
        return Ok(ObjectSet::default());
    };
    let palettes = match vfs.read(&format!("{name}.pl"))? {
        Some(p) => Palette::parse_many(&p)?,
        None => Vec::new(),
    };
    Ok(objects::load_set(name, &rs2, &palettes).unwrap_or_default())
}

pub fn load(data_dir: &Path, map_path: &Path, climate: &str, vehicles: &[&str]) -> Result<Assets, Error> {
    let vfs = mount(data_dir, climate)?;
    let map = map::parse(&std::fs::read(map_path)?)?;
    let object_sets = map.object_sets.iter().map(|n| object_set(&vfs, n)).collect::<Result<_, _>>()?;
    Ok(Assets {
        projection: Projection::of(&map),
        rhombs: vfs.require("mappic.rs2")?,
        rhomb_palettes: Palette::parse_many(&vfs.require("rhomb.pl")?)?,
        object_sets,
        vehicles: vehicles.iter().filter_map(|n| vehicle(&vfs, n).ok()).collect(),
        curves: curves::parse(&vfs.require("curvenew")?)?.0.iter().map(|c| c.iter().map(|p| (p.x, p.y)).collect()).collect(),
        map,
    })
}
