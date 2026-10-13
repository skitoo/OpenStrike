//! Terrain built from a real install. Skipped when the game data is absent.

use std::path::PathBuf;

use openstrike::{assets, terrain};
use openstrike_formats::map::GROUND_WATER;
use openstrike_formats::objects::{BRIDGE_OPEN, BRIDGE_PART, BRIDGE_SIZE, FOOTPRINT_ORIGIN, FOOTPRINT_SOLID, bridge_tile};
use openstrike_sim::path;

fn data_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("OPENSTRIKE_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/SuddenStrikeGold"));
    dir.join("units.sue").exists().then_some(dir)
}

#[test]
fn water_and_houses_are_blocked_in_the_stored_terrain() {
    let Some(dir) = data_dir() else { return };
    let a = assets::load(&dir, &dir.join("Maps/single/'Act of Sabotage'.ssm"), "summer", &[]).unwrap();
    let t = terrain::build(&a.map);
    for cy in 0..a.map.height {
        for cx in 0..a.map.width {
            if a.map.ground_type(cx, cy) == GROUND_WATER {
                assert!(!t.passable(2 * cx as i32, 2 * cy as i32), "water cell {cx},{cy} is open");
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
                    assert!(!t.passable(tx, ty), "house tile {tx},{ty} is open");
                }
            }
        }
    }
}

#[test]
fn bridges_open_their_deck_as_the_original_rule_says() {
    // Game_Dll.dll 0x100516e7: a grid tile with bit 0x01 is open when BRIDGE_OPEN << state is set
    let Some(dir) = data_dir() else { return };
    let a = assets::load(&dir, &dir.join("Maps/single/'Guard on the river Rhine'.ssm"), "summer", &[]).unwrap();
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
            if !t.contains(tx, ty) {
                continue;
            }
            let open = v & (BRIDGE_OPEN << o.extra[0]) != 0;
            agree += (t.passable(tx, ty) == open) as u32;
            total += 1;
        }
    }
    assert!(total > 1000 && agree * 100 >= total * 97, "{agree}/{total}");
}

#[test]
fn tanks_can_cross_the_rhine_bridges() {
    let Some(dir) = data_dir() else { return };
    let map_path = dir.join("Maps/single/'Guard on the river Rhine'.ssm");
    let a = assets::load(&dir, &map_path, "summer", &["t-26"]).unwrap();
    let t = terrain::build(&a.map);
    let most = a.map.object_sets.iter().position(|n| n == "most").unwrap();
    let mut crossings = 0;
    for o in a.map.objects.iter().filter(|o| o.set as usize == most) {
        let Some(grid) = a.object_sets[most].bridges[o.frame as usize].as_ref() else { continue };
        // the intact deck of this bridge, placed with the original formula
        let deck: Vec<(i32, i32)> = grid
            .iter()
            .enumerate()
            .filter(|&(_, &v)| v & BRIDGE_PART != 0 && v & (BRIDGE_OPEN << o.extra[0]) != 0)
            .map(|(i, _)| bridge_tile(o.x, o.y, (i % BRIDGE_SIZE) as i32, (i / BRIDGE_SIZE) as i32))
            .collect();
        let water = |&(x, y): &(i32, i32)| a.map.ground_type((x / 2) as u32, (y / 2) as u32) == GROUND_WATER;
        if !deck.iter().any(water) {
            continue; // not over water
        }
        let (from, to) = deck
            .iter()
            .flat_map(|&p| deck.iter().map(move |&q| (p, q)))
            .max_by_key(|&(p, q)| (p.0 - q.0).pow(2) + (p.1 - q.1).pow(2))
            .unwrap();
        let route = path::find(&t, from, to).expect("no route over the bridge");
        assert!(route.iter().any(water), "the route does not cross the water");
        crossings += 1;
    }
    // most bridge objects of this map are viaduct spans over land; two road spans cross the river
    assert!(crossings >= 2, "{crossings} bridges crossed");
}

#[test]
fn the_platoon_loads() {
    let Some(dir) = data_dir() else { return };
    let map_path = dir.join("Maps/single/'Act of Sabotage'.ssm");
    let names = ["t-26", "t-34", "kv-1", "tiger", "panther", "sherm-5"];
    let a = assets::load(&dir, &map_path, "summer", &names).unwrap();
    let loaded: Vec<&str> = a.vehicles.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(loaded, names);
}
