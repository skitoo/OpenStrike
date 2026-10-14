//! Terrain built from a real install. Skipped when the game data is absent.

use std::path::PathBuf;

use openstrike::{assets, mission, terrain};
use openstrike_formats::map::GROUND_WATER;
use openstrike_formats::objects::{BRIDGE_OPEN, BRIDGE_PART, BRIDGE_SIZE, FOOTPRINT_ORIGIN, FOOTPRINT_SOLID, bridge_tile};
use openstrike_sim::{Command, Order, Rules, Unit, UnitId, World, step};

fn data_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("OPENSTRIKE_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/SuddenStrikeGold"));
    dir.join("units.sue").exists().then_some(dir)
}

#[test]
fn water_and_houses_are_blocked_in_the_stored_terrain() {
    let Some(dir) = data_dir() else { return };
    let a = assets::load(&dir, &dir.join("Maps/single/'Act of Sabotage'.ssm"), None, &[]).unwrap();
    let t = terrain::build(&a.map);
    for cy in 0..a.map.height {
        for cx in 0..a.map.width {
            if a.map.ground_type(cx, cy) == GROUND_WATER {
                assert!(t.quarter_blocked(2 * cx as i32, 2 * cy as i32), "water cell {cx},{cy} is open");
            }
        }
    }
    let dom = a.map.object_sets.iter().position(|n| n == "dom").unwrap();
    for o in a.map.objects.iter().filter(|o| o.set as usize == dom) {
        let grid = a.object_sets[dom].footprints[o.frame as usize].unwrap();
        for (gy, row) in grid.iter().enumerate() {
            for (gx, v) in row.iter().enumerate() {
                if v & FOOTPRINT_SOLID != 0 {
                    let (tx, ty) = ((o.x as i32 >> 4) + gx as i32 - FOOTPRINT_ORIGIN, (o.y as i32 >> 4) + gy as i32 - FOOTPRINT_ORIGIN);
                    assert!(t.quarter_blocked(tx, ty), "house tile {tx},{ty} is open");
                }
            }
        }
    }
}

#[test]
fn bridges_open_their_deck_as_the_original_rule_says() {
    // Game_Dll.dll 0x100516e7: a grid tile with bit 0x01 is open when BRIDGE_OPEN << state is set
    let Some(dir) = data_dir() else { return };
    let a = assets::load(&dir, &dir.join("Maps/single/'Guard on the river Rhine'.ssm"), None, &[]).unwrap();
    let t = terrain::build(&a.map);
    let most = a.map.object_sets.iter().position(|n| n == "most").unwrap();
    let (mut agree, mut total) = (0, 0);
    for o in a.map.objects.iter().filter(|o| o.set as usize == most) {
        let Some(grid) = a.object_sets[most].bridges[o.frame as usize].as_ref() else { continue };
        for (i, &v) in grid.iter().enumerate() {
            if v & BRIDGE_PART == 0 {
                continue;
            }
            let (tx, ty) = bridge_tile(o.x, o.y, (i % BRIDGE_SIZE) as i32, (i / BRIDGE_SIZE) as i32);
            // the border cells are forced blocked by the game's loader
            if tx < 8 || ty < 8 || tx >= 2 * t.width - 8 || ty >= 2 * t.height - 8 {
                continue;
            }
            let open = v & (BRIDGE_OPEN << o.extra[0]) != 0;
            agree += (!t.quarter_blocked(tx, ty) == open) as u32;
            total += 1;
        }
    }
    assert!(total > 1000 && agree * 100 >= total * 97, "{agree}/{total}");
}

#[test]
fn a_tank_crosses_the_rhine() {
    // a T-26 ordered from the west bank to the east bank, with the game's own movement and path
    // finder: the river runs north-south through the middle, crossed by bridges around y = 30-56
    let Some(dir) = data_dir() else { return };
    let map_path = dir.join("Maps/single/'Guard on the river Rhine'.ssm");
    let a = assets::load(&dir, &map_path, None, &["t-26"]).unwrap();
    let t26 = a.vehicles.iter().find(|v| v.name == "t-26").unwrap();
    let rules = Rules { grid: terrain::build(&a.map), curves: a.curves.clone(), types: vec![t26.unit_type.clone()] };
    let free = |x: i16, y: i16| rules.grid.at(x as i32, y as i32) & 0x400f == 0;
    let near = |x: i16, y: i16| (0..10).flat_map(|d| [(x + d, y), (x - d, y), (x, y + d), (x, y - d)]).find(|&(x, y)| free(x, y)).unwrap();
    let (from, to) = (near(30, 44), near(100, 48));
    let mut w = step(&World::new(vec![Unit::new(UnitId(1), 0, from, 0)], 0), &rules, &[Command::Move { units: vec![UnitId(1)], to }]);
    let mut seen_east_of_river = false;
    for _ in 0..20_000 {
        w = step(&w, &rules, &[]);
        let c = w.units[0].cell;
        assert!(free(c.0, c.1), "drove onto a blocked cell {c:?}");
        seen_east_of_river |= c.0 > 86;
        if w.units[0].order == Order::Idle {
            break;
        }
    }
    assert!(seen_east_of_river, "stayed on the west bank, at {:?}", w.units[0].cell);
    assert_eq!(w.units[0].cell, to);
}

#[test]
fn extra_vehicles_load_after_the_maps_own() {
    let Some(dir) = data_dir() else { return };
    let map_path = dir.join("Maps/single/'Act of Sabotage'.ssm");
    let names = ["t-26", "t-34", "kv-1"];
    let a = assets::load(&dir, &map_path, None, &names).unwrap();
    let loaded: Vec<&str> = a.vehicles.iter().map(|v| v.name.as_str()).collect();
    assert!(loaded.ends_with(&names), "{loaded:?}");
    // speeds in 1/256 px per tick, from MoveSpeed (second value, scaled and truncated)
    let speeds: Vec<i32> = a.vehicles[loaded.len() - 3..].iter().map(|v| v.unit_type.speed(0x8000)).collect();
    assert_eq!(speeds, [384, 371, 243]);
}

#[test]
fn the_mission_starts_with_its_own_tanks() {
    let Some(dir) = data_dir() else { return };
    let a = assets::load(&dir, &dir.join("Maps/single/'Act of Sabotage'.ssm"), None, &[]).unwrap();
    let kinds = a.vehicles.iter().enumerate().map(|(i, v)| (v.name.clone(), i)).collect();
    let start = mission::start(&a.map, &a.unit_classes, &kinds, &a.vehicles.iter().map(|v| v.unit_type.clone()).collect::<Vec<_>>());
    let tanks = a.map.units.iter().filter(|u| a.unit_classes[&a.map.unit_types[u.kind as usize].to_ascii_lowercase()] == "tank").count();
    assert_eq!(start.world.units.len(), tanks);
    assert_eq!(start.skipped.values().sum::<usize>() + tanks, a.map.units.len());
    // the first unit of the file (a ship gun of player 3) gets id 3 * 1024 + 1 and ends up last
    let last = start.world.units.last().unwrap();
    assert_eq!((last.id, last.player, last.cell, last.facing), (UnitId(3 * 1024 + 1), 3, (720 >> 5, 3792 >> 5), 16));
    assert_eq!(last.crew, 0);
}

#[test]
fn the_first_archive_listed_in_sudtest_ini_wins() {
    let Some(dir) = data_dir() else { return };
    let archive = |name: &str| openstrike_formats::sue::Archive::parse(std::fs::read(dir.join(name)).unwrap()).unwrap();
    let beach = assets::mount(&dir, "beach").unwrap();
    // DataPathScm3_0=beach.sue;summer.sue;...: the beach ground, not the summer one
    assert_eq!(beach.require("mappic.rs2").unwrap(), archive("beach.sue").read("mappic.rs2").unwrap().unwrap());
    assert_ne!(beach.require("mappic.rs2").unwrap(), archive("summer.sue").read("mappic.rs2").unwrap().unwrap());
    // units.sue comes before animats.sue
    assert_eq!(beach.require("str.col").unwrap(), archive("units.sue").read("str.col").unwrap().unwrap());
}

#[test]
fn the_group_digits_are_the_interface_font() {
    let Some(dir) = data_dir() else { return };
    let a = assets::load(&dir, &dir.join("Maps/single/'Act of Sabotage'.ssm"), None, &[]).unwrap();
    assert_eq!(a.group_digits.len(), 10);
    // 4 x 5 pixels hanging 5 above their origin, "1" narrower
    assert_eq!((a.group_digits[0].width, a.group_digits[0].height, a.group_digits[0].anchor), (4, 5, (0, -5)));
    assert_eq!(a.group_digits[1].width, 3);
    assert!(a.group_digits.iter().all(|d| d.rgba.chunks(4).any(|p| p[3] == 255)));
}

#[test]
fn the_orders_grid_lies_on_the_panel() {
    let Some(dir) = data_dir() else { return };
    let a = assets::load(&dir, &dir.join("Maps/single/'Act of Sabotage'.ssm"), None, &[]).unwrap();
    assert_eq!(a.layout[0x20], [266, 5, 290, 32]);
    assert_eq!(a.layout[0x28], [322, 67, 346, 94]);
    let b = &a.buttons[&51];
    assert_eq!((b.width, b.height), (25, 28));
    assert_eq!((a.buttons[&29].width, a.buttons[&32].width), (40, 32));
}
