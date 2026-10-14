//! The mission's starting units, created as the game does from the map's unit section
//! (`Game_Dll.dll` `0x1003fe70`): in file order, each gets the first free id of its player from
//! `player * 1024 + 1` on (`0x1003e370`) and is put at the head of the unit list, so the list (the
//! update order) is the reverse of the file. Only the `tank` class is simulated so far; units of
//! the other classes still take their ids but are left out.

use std::collections::{BTreeMap, HashMap};

use openstrike_formats::map::{Map, MapPlayer};
use openstrike_sim::{CREW_FULL, Unit, UnitId, UnitType, World};

/// The human player: in single player the game makes slot 0 the only human one (`Game_Dll.dll`
/// `0x1004f320`, reached from `0x1004f4e0` unless `[0x10388464]`, a flag from the launcher, is 1;
/// then the slots come from `XCHNG\TOGAME\mplay_info`, `0x1004f060`, as in multiplayer).
pub const HUMAN_PLAYER: u8 = 0;

/// The class OpenStrike simulates (tank unit vtable `0x100a83f0`).
pub const TANK: &str = "tank";

/// Crew condition from the map's percentage (`0x10040030`: `(p << 15) / 100` by multiplication,
/// at most [`CREW_FULL`]).
pub fn crew_condition(percent: u8) -> i32 {
    ((((percent as i64) << 15) * 0x51eb_851f) >> 37).min(CREW_FULL as i64) as i32
}

/// Sides friendly to each side in single player (`Game_Dll.dll` `0x1004f320`): 0 and 2; side 3
/// (every map's "Neutral") is in no table.
const FRIENDS: [u8; 4] = [0b101, 0b010, 0b101, 0];
/// Sides hostile to each side in single player: 1 to 0 and 2, and the other way round.
const FOES: [u8; 4] = [0b010, 0b101, 0b010, 0];

/// How player `p` sees player `q` (`0x1004ef20`: from their sides, `[0x101ff3ac + 2 * p]` gets
/// the friends' bits and `[0x101ff390 + 2 * p]` the others' that are hostile). The map lists the
/// players with their side; a player it does not list is on side 0.
pub fn hostile(players: &[MapPlayer], p: u8, q: u8) -> bool {
    let side = |p: u8| players.get(p as usize).map_or(0, |pl| pl.side as usize);
    let (sp, sq) = (side(p), side(q));
    let bit = 1u8.checked_shl(sq as u32).unwrap_or(0);
    let row = |t: &[u8; 4]| t.get(sp).copied().unwrap_or(0);
    row(&FRIENDS) & bit == 0 && row(&FOES) & bit != 0
}

/// Hit points and rounds from the map's percentages (`0x10040030`): `max(1, health% * Health /
/// 100)` and `ammo% * Ammo1 / 100`, likewise for `Ammo2`.
pub fn supplies(t: &UnitType, health: u8, ammo: u8) -> (i32, [i32; 2]) {
    let hp = (health as i32 * t.health / 100).max(1);
    (hp, t.ammo.map(|max| ammo as i32 * max / 100))
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
/// lowercase type names to their index in `types`, the simulation's types.
pub fn start(map: &Map, classes: &HashMap<String, String>, kinds: &HashMap<String, usize>, types: &[UnitType]) -> Start {
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
                (unit.hp, unit.ammo) = supplies(&types[kind], u.health, u.ammo);
                created.push(unit);
            }
            _ => *skipped.entry(class.to_string()).or_insert(0) += 1,
        }
    }
    created.reverse();
    Start { world: World::new(created, 0), skipped }
}
