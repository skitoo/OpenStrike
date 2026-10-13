//! Terrain built from a real install. Skipped when the game data is absent.

use std::path::PathBuf;

use openstrike::{assets, terrain};
use openstrike_formats::map::GROUND_WATER;
use openstrike_formats::objects::BRIDGE_ORIGIN;
use openstrike_sim::path;
use openstrike_sim::terrain::TILE;

fn data_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("OPENSTRIKE_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/SuddenStrikeGold"));
    dir.join("units.sue").exists().then_some(dir)
}

#[test]
fn tanks_can_cross_the_rhine_bridges() {
    let Some(dir) = data_dir() else { return };
    let map_path = dir.join("Maps/single/'Guard on the river Rhine'.ssm");
    let a = assets::load(&dir, &map_path, "summer", &["t-26"]).unwrap();
    let t = terrain::build(&a.map, &a.object_sets);
    let most = a.map.object_sets.iter().position(|n| n == "most").unwrap();
    let mut crossings = 0;
    for o in a.map.objects.iter().filter(|o| o.set as usize == most) {
        let Some(deck) = a.object_sets[most].decks[o.frame as usize].as_ref() else { continue };
        let origin = ((o.x as i32).div_euclid(TILE) - BRIDGE_ORIGIN.0, (o.y as i32).div_euclid(TILE) - BRIDGE_ORIGIN.1);
        let tiles: Vec<(i32, i32)> = deck.iter().map(|&(x, y)| (origin.0 + x, origin.1 + y)).collect();
        let water = |&(x, y): &(i32, i32)| a.map.ground_type((x / 2) as u32, (y / 2) as u32) == GROUND_WATER;
        if !tiles.iter().any(water) {
            continue; // not over water
        }
        // the two deck tiles furthest apart: one on each bank
        let (from, to) = tiles
            .iter()
            .flat_map(|&p| tiles.iter().map(move |&q| (p, q)))
            .max_by_key(|&(p, q)| (p.0 - q.0).pow(2) + (p.1 - q.1).pow(2))
            .unwrap();
        let route = path::find(&t, from, to).expect("no route over the bridge");
        assert!(route.iter().any(water), "the route does not cross the water");
        crossings += 1;
    }
    assert!(crossings >= 4, "{crossings} bridges crossed");
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
