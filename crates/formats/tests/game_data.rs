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
