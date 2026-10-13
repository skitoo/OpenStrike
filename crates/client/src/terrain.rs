//! Passability of a map for the simulation, built from the formats' data.

use openstrike_formats::map::{CELL, GROUND_WATER, Map};
use openstrike_formats::objects::{BRIDGE_ORIGIN, FOOTPRINT_ORIGIN, FOOTPRINT_SOLID, ObjectSet};
use openstrike_sim::Terrain;
use openstrike_sim::terrain::TILE;

/// Water cells and the solid part of building footprints are blocked; bridge decks are open,
/// even over water.
pub fn build(map: &Map, sets: &[ObjectSet]) -> Terrain {
    let per_cell = CELL as i32 / TILE;
    let (w, h) = (map.width as i32 * per_cell, map.height as i32 * per_cell);
    let water = (0..map.height).flat_map(|y| (0..map.width).map(move |x| (x, y))).filter(|&(x, y)| map.ground_type(x, y) == GROUND_WATER);
    let water_tiles = water.flat_map(|(x, y)| {
        (0..per_cell).flat_map(move |dy| (0..per_cell).map(move |dx| (x as i32 * per_cell + dx, y as i32 * per_cell + dy)))
    });
    let buildings = map.objects.iter().flat_map(|o| {
        let footprint = sets.get(o.set as usize).and_then(|s| s.footprints.get(o.frame as usize)).copied().flatten();
        let origin = ((o.x as i32).div_euclid(TILE) - FOOTPRINT_ORIGIN, (o.y as i32).div_euclid(TILE) - FOOTPRINT_ORIGIN);
        footprint.into_iter().flat_map(move |grid| {
            grid.into_iter().enumerate().flat_map(move |(gy, row)| {
                row.into_iter()
                    .enumerate()
                    .filter(|&(_, v)| v & FOOTPRINT_SOLID != 0)
                    .map(move |(gx, _)| (origin.0 + gx as i32, origin.1 + gy as i32))
            })
        })
    });
    let decks = map.objects.iter().flat_map(|o| {
        let deck = sets.get(o.set as usize).and_then(|s| s.decks.get(o.frame as usize)).cloned().flatten();
        let origin = ((o.x as i32).div_euclid(TILE) - BRIDGE_ORIGIN.0, (o.y as i32).div_euclid(TILE) - BRIDGE_ORIGIN.1);
        deck.into_iter().flatten().map(move |(x, y)| (origin.0 + x, origin.1 + y))
    });
    Terrain::open(w, h).with_blocked(water_tiles.chain(buildings)).with_unblocked(decks)
}
