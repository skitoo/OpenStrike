//! Checks against a real Sudden Strike Gold install. Skipped when the data is absent:
//! set `OPENSTRIKE_DATA` or put the game in `data/SuddenStrikeGold` at the workspace root.

use std::path::PathBuf;

use openstrike_formats::{map, palette::Palette, sprite, sue::Archive};

fn data_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("OPENSTRIKE_DATA").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/SuddenStrikeGold")
    });
    dir.join("units.sue").exists().then_some(dir).or_else(|| {
        eprintln!("game data not found, skipping");
        None
    })
}

#[test]
fn every_archive_entry_decompresses() {
    let Some(dir) = data_dir() else { return };
    let mut files = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("sue")) {
            let archive = Archive::parse(std::fs::read(&path).unwrap()).unwrap();
            let names: Vec<String> = archive.names().map(String::from).collect();
            for name in names {
                archive.read(&name).unwrap().unwrap();
                files += 1;
            }
        }
    }
    assert_eq!(files, 2943);
}

#[test]
fn unit_sprites_decode() {
    let Some(dir) = data_dir() else { return };
    let units = Archive::parse(std::fs::read(dir.join("units.sue")).unwrap()).unwrap();
    let pck = units.read("t-26.pck").unwrap().unwrap();
    let pal = Palette::parse(&units.read("t-26.col").unwrap().unwrap()).unwrap();
    let frames = sprite::pck_frames(&pck).unwrap();
    assert_eq!(frames.len(), 288);
    for f in frames {
        sprite::decode(f, &pal).unwrap();
    }
    let first = sprite::decode(sprite::pck_frames(&pck).unwrap()[0], &pal).unwrap();
    assert_eq!((first.anchor, first.width, first.height), ((-24, -20), 48, 29));
}

#[test]
fn every_single_player_map_parses() {
    let Some(dir) = data_dir() else { return };
    let mut count = 0;
    for entry in std::fs::read_dir(dir.join("Maps/single")).unwrap() {
        let path = entry.unwrap().path();
        let m = map::parse(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(m.object_sets.len(), 11);
        assert!(m.objects.iter().all(|o| (o.set as usize) < m.object_sets.len()));
        count += 1;
    }
    assert_eq!(count, 164);
}

#[test]
fn summer_object_sets_decode() {
    use openstrike_formats::objects::{self, Layer};
    let Some(dir) = data_dir() else { return };
    let summer = Archive::parse(std::fs::read(dir.join("summer.sue")).unwrap()).unwrap();
    for name in ["trop", "tree", "stand", "zabor", "dom", "lay", "fre", "cliff", "most"] {
        let stem = objects::file_stem(name);
        let rs2 = summer.read(&format!("{stem}.rs2")).unwrap().unwrap();
        let pals = summer.read(&format!("{name}.pl")).unwrap().map(|p| Palette::parse_many(&p).unwrap()).unwrap_or_default();
        let set = objects::load_set(name, &rs2, &pals).unwrap();
        let decoded = set.frames.iter().filter(|f| !f.states.is_empty()).count();
        assert!(decoded > 0, "{name}: nothing decoded");
        if name == "tree" {
            // standing tree then its shadow
            let layers: Vec<Layer> = set.frames[0].parts(0).iter().map(|p| p.layer).collect();
            assert_eq!(layers, vec![Layer::Upright, Layer::Shadow]);
        }
    }
}

#[test]
fn water_cells_are_ground_type_water() {
    let Some(dir) = data_dir() else { return };
    let m = map::parse(&std::fs::read(dir.join("Maps/single/'Act of Sabotage'.ssm")).unwrap()).unwrap();
    let water = (0..m.height)
        .flat_map(|y| (0..m.width).map(move |x| (x, y)))
        .filter(|&(x, y)| m.ground_type(x, y) == map::GROUND_WATER)
        .count();
    // the harbour: about a quarter of the 128x128 cells
    assert!((3000..6000).contains(&water), "{water} water cells");
}

#[test]
fn house_footprints_have_a_solid_core() {
    use openstrike_formats::objects::{self, FOOTPRINT_SOLID};
    let Some(dir) = data_dir() else { return };
    let summer = Archive::parse(std::fs::read(dir.join("summer.sue")).unwrap()).unwrap();
    let set = objects::load_set("dom", &summer.read("houses.rs2").unwrap().unwrap(), &[]).unwrap();
    let grid = set.footprints[0].unwrap();
    let solid: usize = grid.iter().flatten().filter(|&&v| v & FOOTPRINT_SOLID != 0).count();
    assert!((10..60).contains(&solid), "{solid} solid cells");
    // the cell under the object position is inside the building
    assert_ne!(grid[6][6] & FOOTPRINT_SOLID, 0);
}

#[test]
fn counted_unit_sprites_decode() {
    let Some(dir) = data_dir() else { return };
    let units = Archive::parse(std::fs::read(dir.join("units.sue")).unwrap()).unwrap();
    // para.pck is another layout (its header says 21 frames, its table holds 43 offsets): not decoded yet
    for name in ["shot", "str"] {
        let pck = units.read(&format!("{name}.pck")).unwrap().unwrap();
        let frames = sprite::pck_frames(&pck).unwrap();
        assert!(frames.iter().all(|f| f.is_empty() || sprite::frame_len(f, 0) == Some(f.len())), "{name}: frames do not parse");
    }
}

#[test]
fn bridges_have_four_states() {
    use openstrike_formats::objects::{self, Layer};
    let Some(dir) = data_dir() else { return };
    let summer = Archive::parse(std::fs::read(dir.join("summer.sue")).unwrap()).unwrap();
    let set = objects::load_set("most", &summer.read("most.rs2").unwrap().unwrap(), &[]).unwrap();
    let bridge = &set.frames[32];
    assert_eq!(bridge.states.len(), 4);
    for state in &bridge.states {
        let layers: Vec<Layer> = state.iter().map(|p| p.layer).collect();
        assert_eq!(layers, vec![Layer::Flat, Layer::Shadow, Layer::Flat, Layer::Shadow]);
    }
    // every state shares the abutments but has its own deck
    assert_eq!(bridge.states[0][0].image, bridge.states[3][0].image);
    assert_ne!(bridge.states[0][2].image, bridge.states[3][2].image);
}

#[test]
fn trees_stands_and_houses_have_their_states() {
    use openstrike_formats::objects::{self, Layer, map_state};
    let Some(dir) = data_dir() else { return };
    let summer = Archive::parse(std::fs::read(dir.join("summer.sue")).unwrap()).unwrap();
    let pals = |n: &str| Palette::parse_many(&summer.read(&format!("{n}.pl")).unwrap().unwrap()).unwrap();
    let trees = objects::load_set("tree", &summer.read("tree.rs2").unwrap().unwrap(), &pals("tree")).unwrap();
    // a standing tree, and the same tree down: its shadow and the fallen trunk on the ground
    assert_eq!(trees.frames[0].states.len(), 2);
    let down: Vec<Layer> = trees.frames[0].parts(1).iter().map(|p| p.layer).collect();
    assert!(down.contains(&Layer::Flat) && !down.contains(&Layer::Upright), "{down:?}");
    let houses = objects::load_set("dom", &summer.read("houses.rs2").unwrap().unwrap(), &[]).unwrap();
    assert!(houses.frames.iter().filter(|f| !f.states.is_empty()).all(|f| f.states.len() == 4));
    assert_eq!(map_state("stand", 3), 1);
    assert_eq!(map_state("tree", 1), 1);
    assert_eq!(map_state("dom", 2), 2);
    assert_eq!(map_state("zabor", 2), 0);
}

#[test]
fn movement_curves_parse() {
    let Some(dir) = data_dir() else { return };
    let common = Archive::parse(std::fs::read(dir.join("common.sue")).unwrap()).unwrap();
    let curves = openstrike_formats::curves::parse(&common.read("curvenew").unwrap().unwrap()).unwrap();
    assert_eq!(curves.0.len(), 81);
    // straight on east: one point on the west edge of the new cell, 15 px down
    assert_eq!(curves.get(0, 0).iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(), vec![(0, 3840)]);
    // stopping: the middle of the cell
    assert_eq!(curves.get(8, 0).last().map(|p| (p.x, p.y)), Some((3840, 3840)));
    assert!(curves.get(8, 8).is_empty());
}

#[test]
fn unit_descriptions_follow_their_includes() {
    let Some(dir) = data_dir() else { return };
    let desc_common = Archive::parse(std::fs::read(dir.join("desc_common.sue")).unwrap()).unwrap();
    let desc = openstrike_formats::desc::load("units/t-26", &mut |name| desc_common.read(name).ok().flatten()).unwrap();
    // MoveSpeed 1.2,1.4 in light.tank.common, overridden by the unit's own 1.4,1.5
    assert_eq!((desc.float("MoveSpeed", 0), desc.float("MoveSpeed", 1)), (Some(1.4), Some(1.5)));
    assert_eq!((desc.int("TurnDelay", 0), desc.int("TurnDelay", 1)), (Some(5), Some(3)));
    assert_eq!(desc.int("CanMoveBackward", 0), Some(1));
    assert_eq!(desc.get("Engine").map(|v| v[0].as_str()), Some("Track"));
}
