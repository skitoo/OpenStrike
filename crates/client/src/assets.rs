//! Loading game data into plain values, before anything touches the GPU.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use openstrike_formats::ini::Ini;
use openstrike_formats::iso::Projection;
use openstrike_formats::map::{self, Map};
use openstrike_formats::objects::{self, ObjectSet};
use openstrike_formats::palette::Palette;
use openstrike_formats::sprite::{self, Image, KIND_SHADOW};
use openstrike_formats::sue::{Archive, Vfs};
use openstrike_formats::{circles, curves, desc, hot, interface};
use openstrike_sim::UnitType;

use crate::mission;

pub type Error = Box<dyn std::error::Error>;

/// Sprites of a turreted vehicle, each strip holding the 32 headings.
pub struct VehicleSprites {
    pub hull: Vec<Image>,
    /// Tracks and running gear (frames 128-159), drawn before the shadows: `Game_Dll.dll`
    /// `0x1000ee30` reads frame `128 + heading` from the type's frame table (`+0xea6`).
    pub tracks: Vec<Image>,
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
    /// `Name`, the name the information bar shows (the type's `+0x24`).
    pub title: String,
    pub unit_type: UnitType,
    /// `Selector`: half the width of the status bars drawn under the unit when selected (`+0xb4`,
    /// read by `0x10046d2f`; 1 by default).
    pub selector: i32,
    pub sprites: VehicleSprites,
}

pub struct Assets {
    pub map: Map,
    pub projection: Projection,
    pub rhombs: Vec<u8>,
    pub rhomb_palettes: Vec<Palette>,
    /// One per entry of `map.object_sets`; empty when the set could not be loaded.
    pub object_sets: Vec<ObjectSet>,
    /// The `tank` class types the map's units use (in the order of [`Map::unit_types`]), then
    /// the extra ones asked for; those that could not be loaded are left out.
    pub vehicles: Vec<Vehicle>,
    /// Class of each unit type (lowercase names, `desc_common/units/UNITS`).
    pub unit_classes: HashMap<String, String>,
    /// The interface panel picture (`intrpic.dat` frame 1, palette `intr.pl`; loaded by
    /// `Game_Dll.dll` `0x10072fc3`).
    pub panel: Image,
    /// The interface colours (`user.pl`, 256 RGB565 values; loaded by `Game_Dll.dll`
    /// `0x10082240` into `0x10387cb0`).
    pub user_colours: Palette,
    /// The group numbers over the status bars: glyphs `0x30`-`0x39` of `intrpic.dat` frame 13, the
    /// one for group value `v` (1-10, key `v - 1`) at `v - 1` (`Game_Dll.dll` `0x10009ff0` draws glyph
    /// `v + 0x2f`), in the palette of `user.pl` from entry 62 (`0x10387d2c`).
    pub group_digits: Vec<Image>,
    /// The panel's layout (`intrpic.dat` frame 0): 16-byte records of x0, y0, x1, y1 in the panel.
    pub layout: Vec<[i32; 4]>,
    /// The pictures of the panel's buttons, `intrpic.dat` frames 29-86 by frame.
    pub buttons: HashMap<usize, Image>,
    /// The information bar (`intrpic.dat` frame 5, the `NWTT` window of `0x10077af0`).
    pub info_bar: Image,
    /// The interface font (`intrpic.dat` frame 11, palette `intr.pl`), a glyph per character code.
    pub font: Vec<Option<Image>>,
    /// The information bar's layout (`intrpic.dat` frame 4).
    pub info_layout: Vec<[i32; 4]>,
    /// The icons of its four fields (`intrpic.dat` frames 7-10).
    pub info_icons: Vec<Image>,
    /// The small font of the fields' values (`intrpic.dat` frame 13, palette `user.pl` from entry 58,
    /// `0x10387d24`), a glyph per character code.
    pub small_font: Vec<Option<Image>>,
    /// `intrtext.dat`.
    pub texts: Vec<String>,
    /// The key names of `key.map` by action, lowercase (`00` -> `A`).
    pub key_names: HashMap<String, String>,
    /// Sight rays by radius (`circ64.dat`).
    pub circles: openstrike_sim::vision::Circles,
    /// `CURVENEW` points per `next * 9 + previous`.
    pub curves: Vec<Vec<(i16, i16)>>,
}

pub const CLIMATES: [&str; 4] = ["beach", "desert", "summer", "winter"];

/// Data scheme of a climate: 1 + the map's climate byte (summer 0, winter 1, beach 2, desert 3).
fn scheme(climate: &str) -> u32 {
    match climate {
        "winter" => 2,
        "beach" => 3,
        "desert" => 4,
        _ => 1,
    }
}

/// A file of the install, whatever the case of its name (the game runs on Windows).
fn find_file(dir: &Path, name: &str) -> Result<PathBuf, Error> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)) {
            return Ok(path);
        }
    }
    Err(format!("{name} not found in {}", dir.display()).into())
}

/// Mount the archives the game mounts for `climate`, read from `SudTest.ini` as the game does
/// ([`Ini::data_paths`] of scheme 0, then of the climate's scheme). The game files every entry in
/// one table when an archive is mounted and keeps the first archive's entry for a name met again
/// (`Game_Dll.dll` `0x10093090`; an archive name starting with `^` would replace it instead), so
/// the first archive listed wins and mounting one again changes nothing. Archives no key names,
/// such as `common01.sue`, `summer01.sue`, `winter01.sue`, `menu01.sue` (older copies of the same
/// data) and the editor's `*_edit.sue`, are never read.
pub fn mount(data_dir: &Path, climate: &str) -> Result<Vfs, Error> {
    let ini = Ini::parse(&String::from_utf8_lossy(&std::fs::read(find_file(data_dir, "SudTest.ini")?)?));
    let mut names: Vec<String> = Vec::new();
    for name in ini.data_paths(0).into_iter().chain(ini.data_paths(scheme(climate))) {
        if name.starts_with('^') {
            return Err(format!("archive {name}: the ^ override prefix is not supported").into());
        }
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    }
    if names.is_empty() {
        return Err("no DataPathScm keys in SudTest.ini".into());
    }
    // later mounts win in the Vfs, so mount the list in reverse
    let mut vfs = Vfs::default();
    for name in names.iter().rev() {
        vfs.mount(Archive::parse(std::fs::read(find_file(data_dir, name)?)?)?);
    }
    Ok(vfs)
}

/// A frame of the interface pictures (`intrpic.dat`, palette `intr.pl`).
fn interface_frame(vfs: &Vfs, index: usize) -> Result<Image, Error> {
    let data = vfs.require("intrpic.dat")?;
    let palette = Palette::parse(&vfs.require("intr.pl")?)?;
    let frames = sprite::pck_frames(&data)?;
    let frame = frames.get(index).ok_or("intrpic.dat: frame out of range")?;
    Ok(sprite::decode(frame, &palette)?)
}

/// See [`Assets::font`]: glyphs that do not decode are left out.
fn font(vfs: &Vfs) -> Result<Vec<Option<Image>>, Error> {
    let data = vfs.require("intrpic.dat")?;
    let palette = Palette::parse(&vfs.require("intr.pl")?)?;
    let frames = sprite::pck_frames(&data)?;
    let font = frames.get(11).ok_or("intrpic.dat: no frame 11")?;
    Ok((0..=255u8)
        .map(|c| sprite::collection_frame(font, c).ok().and_then(|f| sprite::decode(f, &palette).ok()))
        .collect())
}

/// See [`Assets::small_font`].
fn small_font(vfs: &Vfs) -> Result<Vec<Option<Image>>, Error> {
    let user = vfs.require("user.pl")?;
    let mut shifted = user.get(58 * 2..).ok_or("user.pl: too short")?.to_vec();
    shifted.resize(512, 0);
    let palette = Palette::parse(&shifted)?;
    let data = vfs.require("intrpic.dat")?;
    let frames = sprite::pck_frames(&data)?;
    let font = frames.get(13).ok_or("intrpic.dat: no frame 13")?;
    Ok((0..=255u8)
        .map(|c| sprite::collection_frame(font, c).ok().and_then(|f| sprite::decode(f, &palette).ok()))
        .collect())
}

/// See [`Assets::key_names`]: the game reads `key.map` from the install's root.
fn key_names(data_dir: &Path, vfs: &Vfs) -> Result<HashMap<String, String>, Error> {
    let data = match find_file(data_dir, "key.map") {
        Ok(path) => std::fs::read(path)?,
        Err(_) => vfs.require("key.map")?,
    };
    let names = interface::key_names(&String::from_utf8_lossy(&data));
    Ok(names.into_iter().map(|(k, v)| (k.to_ascii_lowercase(), v)).collect())
}

/// A layout of `intrpic.dat` (frame 0 the panel's, 4 the information bar's): 16-byte records of x0,
/// y0, x1, y1.
fn layout(vfs: &Vfs, frame: usize) -> Result<Vec<[i32; 4]>, Error> {
    let data = vfs.require("intrpic.dat")?;
    let frames = sprite::pck_frames(&data)?;
    let frame = frames.get(frame).ok_or("intrpic.dat: no layout frame")?;
    Ok(frame
        .as_chunks::<16>()
        .0
        .iter()
        .map(|r| std::array::from_fn(|i| i32::from_le_bytes(r[4 * i..4 * i + 4].try_into().unwrap())))
        .collect())
}

/// See [`Assets::group_digits`].
fn group_digits(vfs: &Vfs) -> Result<Vec<Image>, Error> {
    let user = vfs.require("user.pl")?;
    let mut shifted = user.get(62 * 2..).ok_or("user.pl: too short")?.to_vec();
    shifted.resize(512, 0);
    let palette = Palette::parse(&shifted)?;
    let data = vfs.require("intrpic.dat")?;
    let fonts = sprite::pck_frames(&data)?;
    let font = fonts.get(13).ok_or("intrpic.dat: no frame 13")?;
    (0x30..=0x39).map(|c| Ok(sprite::decode(sprite::collection_frame(font, c)?, &palette)?)).collect()
}

/// Simulation parameters, sprite file, `Selector` and `Name` of a unit from its description.
pub fn unit_type(vfs: &Vfs, name: &str) -> Result<(UnitType, String, i32, String), Error> {
    let d = desc::load(&format!("units/{name}"), &mut |n| vfs.read(n).ok().flatten())?;
    let missing = |key: &str| format!("{name}: no {key} in the description");
    let speed = (
        d.float("MoveSpeed", 0).ok_or_else(|| missing("MoveSpeed"))?,
        d.float("MoveSpeed", 1).ok_or_else(|| missing("MoveSpeed"))?,
    );
    let turn = (
        d.int("TurnDelay", 0).ok_or_else(|| missing("TurnDelay"))?,
        d.int("TurnDelay", 1).ok_or_else(|| missing("TurnDelay"))?,
    );
    let mut t = UnitType::new(name, speed, turn, d.int("GunTurnDelay", 0).unwrap_or(0));
    t.ignore_mines = d.int("IgnoreMines", 0).unwrap_or(0) != 0;
    t.can_move_backward = d.int("CanMoveBackward", 0).unwrap_or(0) != 0;
    t.health = d.int("Health", 0).unwrap_or(t.health);
    t.ammo = [d.int("Ammo1", 0).unwrap_or(0), d.int("Ammo2", 0).unwrap_or(0)];
    t.move_damage_hp = d.int("MoveDamageHP", 0).unwrap_or(0);
    // `Sight a,b`, each kept in 1-31 (`0x10046e82`)
    if let (Some(a), Some(b)) = (d.int("Sight", 0), d.int("Sight", 1)) {
        let (a, b) = (a.clamp(1, 31), b.clamp(1, 31));
        (t.sight_min, t.sight_range) = (a, b - a);
    }
    let file = d.get("File").and_then(|v| v.first()).ok_or_else(|| missing("File"))?.to_ascii_lowercase();
    Ok((t, file, d.int("Selector", 0).unwrap_or(1), d.string("Name").unwrap_or_default()))
}

pub fn vehicle(vfs: &Vfs, name: &str) -> Result<Vehicle, Error> {
    let (unit_type, file, selector, title) = unit_type(vfs, name)?;
    Ok(Vehicle { name: name.to_string(), title, unit_type, selector, sprites: vehicle_sprites(vfs, &file)? })
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
    mounts.resize(32, (0, 0));
    Ok(VehicleSprites {
        hull: decode(0)?,
        tracks: decode(128)?,
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

/// Load a map and what it needs. `climate` overrides the map's own (see `Map::climate`).
pub fn load(data_dir: &Path, map_path: &Path, climate: Option<&str>, extra_vehicles: &[&str]) -> Result<Assets, Error> {
    let map = map::parse(&std::fs::read(map_path)?)?;
    let vfs = mount(data_dir, climate.unwrap_or(map.climate()))?;
    let object_sets = map.object_sets.iter().map(|n| object_set(&vfs, n)).collect::<Result<_, _>>()?;
    let unit_classes: HashMap<String, String> =
        desc::unit_classes(&String::from_utf8_lossy(&vfs.require("units\\UNITS")?)).into_iter().collect();
    let mut names: Vec<String> = Vec::new();
    for u in &map.units {
        let name = map.unit_types[u.kind as usize].to_ascii_lowercase();
        if unit_classes.get(&name).is_some_and(|c| c == mission::TANK) && !names.contains(&name) {
            names.push(name);
        }
    }
    for n in extra_vehicles {
        if !names.iter().any(|m| m.eq_ignore_ascii_case(n)) {
            names.push(n.to_ascii_lowercase());
        }
    }
    let vehicles = names
        .iter()
        .filter_map(|n| match vehicle(&vfs, n) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("openstrike: vehicle {n} not loaded: {e}");
                None
            }
        })
        .collect();
    Ok(Assets {
        projection: Projection::of(&map),
        rhombs: vfs.require("mappic.rs2")?,
        rhomb_palettes: Palette::parse_many(&vfs.require("rhomb.pl")?)?,
        object_sets,
        vehicles,
        unit_classes,
        panel: interface_frame(&vfs, 1)?,
        user_colours: Palette::parse(&vfs.require("user.pl")?)?,
        group_digits: group_digits(&vfs)?,
        layout: layout(&vfs, 0)?,
        info_layout: layout(&vfs, 4)?,
        info_icons: (7..=10).map(|i| interface_frame(&vfs, i)).collect::<Result<_, _>>()?,
        small_font: small_font(&vfs)?,
        info_bar: interface_frame(&vfs, 5)?,
        font: font(&vfs)?,
        texts: interface::texts(&vfs.require("intrtext.dat")?)?,
        key_names: key_names(data_dir, &vfs)?,
        buttons: (29..=86).map(|i| Ok((i, interface_frame(&vfs, i)?))).collect::<Result<_, Error>>()?,
        circles: circles::parse(&vfs.require("circ64.dat")?)?
            .into_iter()
            .map(|c| c.iter().map(|e| (e.dx, e.dy)).collect())
            .collect(),
        curves: curves::parse(&vfs.require("curvenew")?)?
            .0
            .iter()
            .map(|c| c.iter().map(|p| (p.x, p.y)).collect())
            .collect(),
        map,
    })
}
