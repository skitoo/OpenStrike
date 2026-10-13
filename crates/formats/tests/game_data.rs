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

/// The archives of a climate, as `sudtest.ini` lists them (first listed wins).
fn climate_vfs(dir: &std::path::Path, climate: &str) -> openstrike_formats::sue::Vfs {
    let list: &[&str] = match climate {
        "winter" => &["winter", "desc_winter"],
        "beach" => &["beach", "summer", "desc_summer"],
        "desert" => &["desert", "desc_desert"],
        _ => &["summer", "desc_summer"],
    };
    let mut vfs = openstrike_formats::sue::Vfs::default();
    for name in list.iter().rev() {
        vfs.mount(Archive::parse(std::fs::read(dir.join(format!("{name}.sue"))).unwrap()).unwrap());
    }
    vfs
}

#[test]
fn every_object_of_every_map_has_its_sprites() {
    use openstrike_formats::objects;
    use std::collections::HashMap;
    let Some(dir) = data_dir() else { return };
    let mut sets: HashMap<(String, String), objects::ObjectSet> = HashMap::new();
    let mut vfs_by_climate = HashMap::new();
    let (mut drawn, mut dropped) = (0, 0);
    let mut missing: Vec<String> = Vec::new();
    for e in std::fs::read_dir(dir.join("Maps/single")).unwrap() {
        let path = e.unwrap().path();
        let Ok(m) = map::parse(&std::fs::read(&path).unwrap()) else { continue };
        let climate = m.climate();
        let vfs = vfs_by_climate.entry(climate).or_insert_with(|| climate_vfs(&dir, climate));
        for o in &m.objects {
            let name = m.object_sets[o.set as usize].to_ascii_lowercase();
            let set = sets.entry((climate.to_string(), name.clone())).or_insert_with(|| {
                let Some(rs2) = vfs.read(&format!("{}.rs2", objects::file_stem(&name))).unwrap() else { return Default::default() };
                let pals = vfs.read(&format!("{name}.pl")).unwrap().map(|p| Palette::parse_many(&p).unwrap()).unwrap_or_default();
                objects::load_set(&name, &rs2, &pals).unwrap()
            });
            if objects::placement(&name, o.x, o.y, m.width, m.height).is_none() {
                dropped += 1;
                continue;
            }
            let frame = if name == "zabor" { objects::fence_frame(o.frame, o.extra) } else { o.frame } as usize;
            let state = objects::map_state(&name, o.extra[0]);
            match set.frames.get(frame) {
                Some(f) if !f.parts(state).is_empty() => drawn += 1,
                _ => {
                    if missing.len() < 20 {
                        missing.push(format!("{climate} {name} frame {frame} state {state} ({})", path.file_name().unwrap().to_string_lossy()));
                    }
                }
            }
        }
    }
    assert!(missing.is_empty(), "{drawn} drawn, {dropped} dropped by the game, missing: {missing:#?}");
    assert!(drawn > 900_000, "{drawn} drawn");
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
    use openstrike_formats::objects::{self, LAYER_SHADOW};
    let Some(dir) = data_dir() else { return };
    let summer = Archive::parse(std::fs::read(dir.join("summer.sue")).unwrap()).unwrap();
    let set = objects::load_set("most", &summer.read("most.rs2").unwrap().unwrap(), &[]).unwrap();
    let bridge = &set.frames[32];
    assert_eq!(bridge.states.len(), 4);
    for state in &bridge.states {
        let layers: Vec<u8> = state.iter().map(|p| p.layer).collect();
        assert_eq!(layers, vec![0, LAYER_SHADOW, 0, LAYER_SHADOW]);
    }
    // every state shares the abutments but has its own deck
    assert_eq!(bridge.states[0][0].image, bridge.states[3][0].image);
    assert_ne!(bridge.states[0][2].image, bridge.states[3][2].image);
}

#[test]
fn trees_stands_and_houses_have_their_states() {
    use openstrike_formats::objects::{self, LAYER_UPRIGHT, map_state};
    let Some(dir) = data_dir() else { return };
    let summer = Archive::parse(std::fs::read(dir.join("summer.sue")).unwrap()).unwrap();
    let pals = |n: &str| Palette::parse_many(&summer.read(&format!("{n}.pl")).unwrap().unwrap()).unwrap();
    let trees = objects::load_set("tree", &summer.read("tree.rs2").unwrap().unwrap(), &pals("tree")).unwrap();
    // a standing tree, and the same tree down: its shadow and the fallen trunk on the ground
    assert_eq!(trees.frames[0].states.len(), 2);
    let down: Vec<u8> = trees.frames[0].parts(1).iter().map(|p| p.layer).collect();
    assert!(down.contains(&3) && !down.contains(&LAYER_UPRIGHT), "{down:?}");
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

#[test]
fn fence_frames_follow_their_extra_bytes() {
    let Some(dir) = data_dir() else { return };
    let mut fences = 0;
    for e in std::fs::read_dir(dir.join("Maps/single")).unwrap() {
        let Ok(m) = map::parse(&std::fs::read(e.unwrap().path()).unwrap()) else { continue };
        for o in m.objects.iter().filter(|o| m.object_sets[o.set as usize] == "zabor") {
            assert_eq!(openstrike_formats::objects::fence_frame(o.frame, o.extra), o.frame);
            fences += 1;
        }
    }
    assert_eq!(fences, 190_284);
}

#[test]
fn every_house_reads_as_the_loader_does() {
    let Some(dir) = data_dir() else { return };
    let mut houses = 0;
    for climate in ["summer", "winter", "desert"] {
        let Ok(data) = std::fs::read(dir.join(format!("{climate}.sue"))) else { continue };
        let archive = Archive::parse(data).unwrap();
        let Some(rs2) = archive.read("houses.rs2").unwrap() else { continue };
        let set = openstrike_formats::objects::load_set("dom", &rs2, &[]).unwrap();
        let raw = openstrike_formats::sprite::rs2_frames(&rs2).unwrap();
        for (i, f) in set.frames.iter().enumerate().filter(|&(i, _)| !raw[i].is_empty()) {
            assert_eq!(f.states.len(), 4, "{climate} house {i}");
            houses += 1;
        }
    }
    assert!(houses > 100, "{houses} houses");
}
