//! The mission's starting units, created as the game does from the map's unit section
//! (`Game_Dll.dll` `0x1003fe70`): in file order, each gets the first free id of its player from
//! `player * 1024 + 1` on (`0x1003e370`) and is put at the head of the unit list, so the list (the
//! update order) is the reverse of the file. Only the `tank` class is simulated so far; units of
//! the other classes still take their ids but are left out.

use std::collections::{BTreeMap, HashMap};

use openstrike_formats::map::Map;
use openstrike_sim::{CREW_FULL, Unit, UnitId, World};

/// The human player. **Assumption**, not read in the code: in single player the menu builds the
/// players (`XCHNG\TOGAME\mplay_info`, written by `menu_dll.dll` `0x10018840`) from its session
/// list, whose sides arrive through session messages (`0x1001cd1f`), so the value is only known at
/// run time. Every map names its sides "Spieler", "Gegner", "Verbündeter" (player, enemy, ally),
/// hence player 0. To check against the running game.
pub const HUMAN_PLAYER: u8 = 0;

/// The class OpenStrike simulates (tank unit vtable `0x100a83f0`).
pub const TANK: &str = "tank";

/// Crew condition from the map's percentage (`0x10040030`: `(p << 15) / 100` by multiplication,
/// at most [`CREW_FULL`]).
pub fn crew_condition(percent: u8) -> i32 {
    ((((percent as i64) << 15) * 0x51eb_851f) >> 37).min(CREW_FULL as i64) as i32
}

/// Cell of a vehicle placed at map pixel `(x, y)`: the game moves it to the cell's `(15, 15)`
/// (`0x10040190`), which is where [`Unit::new`] puts a unit.
pub fn vehicle_cell(x: i16, y: i16) -> (i16, i16) {
    (x >> 5, y >> 5)
}

pub struct Start {
    pub world: World,
    /// Units of the classes not simulated yet, counted by class.
    pub skipped: BTreeMap<String, usize>,
}

/// The starting world. `classes` maps lowercase type names to their class (`UNITS`), `kinds`
/// lowercase type names to their index in the simulation's types.
pub fn start(map: &Map, classes: &HashMap<String, String>, kinds: &HashMap<String, usize>) -> Start {
    let mut next_id: HashMap<u8, u32> = HashMap::new();
    let mut created = Vec::new();
    let mut skipped = BTreeMap::new();
    for u in &map.units {
        let id = next_id.entry(u.player).or_insert(u.player as u32 * 1024 + 1);
        let unit_id = UnitId(*id);
        *id += 1;
        let name = map.unit_types[u.kind as usize].to_ascii_lowercase();
        let class = classes.get(&name).map_or("?", String::as_str);
        match kinds.get(&name) {
            Some(&kind) if class == TANK => {
                let mut unit = Unit::new(unit_id, kind, vehicle_cell(u.x, u.y), u.heading & 31);
                unit.player = u.player;
                unit.crew = crew_condition(u.crew_condition);
                created.push(unit);
            }
            _ => *skipped.entry(class.to_string()).or_insert(0) += 1,
        }
    }
    created.reverse();
    Start { world: World::new(created, 0), skipped }
}
