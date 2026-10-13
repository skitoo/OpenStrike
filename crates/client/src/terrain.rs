//! The map as the simulation sees it.

use openstrike_formats::map::Map;
use openstrike_sim::Grid;

/// The cells of the map with the game's own passability bits (water, houses and intact bridges
/// already applied by the editor, see `Map::blocked`) and the blocked border.
pub fn build(map: &Map) -> Grid {
    Grid::from_map(map.width as i32, map.height as i32, &map.layer2)
}
