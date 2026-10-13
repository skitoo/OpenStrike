//! Passability of a map for the simulation.

use openstrike_formats::map::{CELL, Map};
use openstrike_sim::Terrain;
use openstrike_sim::terrain::TILE;

/// The terrain the map stores: the game's own passability bits, with water, houses and intact
/// bridges already applied by the editor (see `Map::blocked`).
pub fn build(map: &Map) -> Terrain {
    let per_cell = CELL as i32 / TILE;
    let (w, h) = (map.width as i32 * per_cell, map.height as i32 * per_cell);
    let blocked = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| map.blocked(x as u32, y as u32));
    Terrain::open(w, h).with_blocked(blocked)
}
