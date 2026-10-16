# Combat fields of unit descriptions

Research for issue #2 (map #1, "Tank skirmish"). Question: which fields of a tank's description, and of
the weapon (`shotN_*`) entries it includes, does the original combat code read; what each one means, its
units, and the address that reads it. And what `crates/formats/src/desc.rs` already handles.

Sources: `Game_Dll.dll` (Sudden Strike Gold, SHA-256 `38f1cdb6…ab19`), read with Ghidra 12.1.4 through REA
and with `llvm-objdump`. Data: `desc_common.sue` (`units/`, `HU/`, `H/`, `shots/`, `HITSNAMES`,
`TARGETTYPES`). Every address is in `Game_Dll.dll`. **Observed** means I read it in the disassembly or
decompilation; **inferred** marks a deduction about purpose or meaning that the code does not state.
"`type + X`" is a field of the unit type object, "`unit + X`" a field of the live unit.

## Summary

- A `tank` type's description is parsed by a chain of four parsers (observed): tank `0x10043030`
  (`GunTurnDelay`, `GunShotWait`) -> gun vehicle `0x10042cd0` (`DaviRange`, `shot1_*`, `shot2_*`) ->
  vehicle `0x10047bd0` / `0x100479c0` / `0x100478d0` (movement, already in `docs/movement.md`) -> base
  type `0x10046b60` (everything shared by all classes: `Health`, `Ammo1/2`, `Sight`, `Armor`,
  `TargetType`, `AttackPref`, `ScanDelay`, `AlarmRange`, `Radius`, `Reload1/2`...). Defaults are set by
  the base constructor `0x100462b0` and the shot-record constructor `0x10047e10`.
- Almost every numeric combat field is a pair `a,b` stored as `a` and `b - a` and read as
  **`a + ((b - a) * k >> 15)`**, where `k` is the unit's crew experience `unit + 0x3d` (0 to `0x8000`),
  raised for veterans (see "Crew factor"). So `a` is the value for a green crew and `b` for a perfect one.
- Two weapons per tank, each a 0x93-byte shot record: `shot1_*` at `type + 0xb70` (main gun) and
  `shot2_*` at `type + 0xc03` (machine gun). Their fields: ID (projectile class), Damage (hit points),
  Range and DeadZone (map pixels, max and min range), Speed (pixels per tick), Accuracy (scatter radius in
  pixels at full range), ReloadTime and BurstReloadTime (ticks), BurstShots (shots), UseAmmo (which
  counter and how many rounds per shot), Expa (experience gained per shot).
- **Armour is not a thickness**: `Armor <hit type> f,f s,s r,r` gives, per hit type (from `HITSNAMES`)
  and per face (front, side, rear), the protection in 1/256 (256 = immune). The game stores
  `256 - value` (clamped to 0..256), i.e. the fraction of damage that gets through. Which face is hit is
  interpolated from the angle between the projectile's heading and the **hull** heading
  (`0x1003f450`). There is no penetration value: `Damage` times the face's damage fraction is the loss.
- Turret: `GunTurnDelay` = ticks between turret steps of 1/32 turn (`0x10010b20`); `GunShotWait` =
  ticks the turret stays frozen after a main gun shot (`0x1000fdf0`).
- `desc.rs` keeps every line and its tokens, so the data is all reachable, but it has no typed view of
  any combat field, `get()` returns only the last line of a key, which loses `Armor` and `AttackPref`
  (one line per hit type / target type), and it does not apply any of the game's load-time transforms.
- **Surprises** (see the end): the `.sue` "raw chunk" rule is wrong and misreads 31 files, two of them
  tank types; `0x100a83f0`, recorded as the tank vtable, is the `apc` unit vtable (tanks use
  `0x100a8d24`); `shotN_Delay` ignores its second value; the machine gun's burst decay uses the main
  gun's reload time.

## Conventions

### The key/value record handed to the parsers

Every parser receives one tokenised line (observed from the offsets they read; the tokeniser itself was
not located): `+0x00` token count (key included), `+0x04 + 4 i` token `i` as an integer, `+0x84 + 4 i`
token `i` as a float, `+0x904 + 0x40 i` token `i` as a string (key at `+0x904`, first value at
`+0x944`). Keys are compared lowercase. Value strings such as `Tank` in `AttackPref Tank 256` are matched
case-sensitively against the lowercase names of `TARGETTYPES` (`0x1004a800`), so the tokeniser must
lowercase them too (inferred). A key with too few values logs "to few params (%i) for %s"
(`0x100b061c`) and is rejected.

### Crew factor

The live unit's `+0x3d` (u16, 0 to `0x8000`) is called "crew condition" in `docs/original-code.md`; it is
raised by experience (`Expa*` keys, `0x10032e90`: `+0x3d = min(0x8000, +0x3d + gain)`), so it is a crew
experience/skill level (inferred name). When the unit has veteran bits (`unit + 0x1a & 0xc000`), every
combat reader uses `k = min(0x8000, max(+0x3d, 0xccd) + (+0x3d >> 1))` instead of `+0x3d`
(observed in `0x10016b30`, `0x100101f0`, `0x10034070`, `0x10017020`, `0x100171e0` and others).
Exceptions noted below read the raw `+0x3d`. The same interpolation is used for the target's armour, with
the **target's** `k`.

### Distances

Unit pixel position = `cell * 32 + (offset >> 8)` on each axis (map pixels, not screen pixels). Combat
distances are octagonal: `max(|dx|, |dy|) + min(|dx|, |dy|) / 2` (unit vtable `+0x90`, `0x100334c0`; also
`0x10041af0`). Some checks use the same formula on cell coordinates (cells).

## Base type fields (parser `0x10046b60`, defaults `0x100462b0`)

| Key | Stored | Meaning and units | Readers |
|---|---|---|---|
| `Health n` | `type + 0xb8` (default 1) | maximum hit points | init `0x10040030` (map health %); `0x1000f716`; repair need `0x10032eb0` |
| `Ammo1 n`, `Ammo2 n` | `+0xbc`, `+0xc0` (default 0) | capacity of the two ammunition counters `unit + 0x39`, `unit + 0x3b` (rounds) | init `0x10040030`; resupply need `0x10032eb0` |
| `Sight a,b` | `+0xc4` = a, `+0xc8` = b - a, each clamped to 1..31 | sight radius in cells (already in `docs/original-code.md`, "Sight") | `0x10034220` |
| `TargetType name` | `+0xcc` (default 0) | index in `TARGETTYPES` (alphabetical file order: ATG, bigeyes, dom, fast, longdistance, man, soft, tank); what this unit counts as when others pick targets | target scoring `0x10010fa2` and others |
| `AttackPref name w` | `+0xd0 + 4 * index` (8 slots, default 0) | target priority weight, in 1/256, per target type | target scoring: `(w + 256) * closeness >> 9`, +1/8 for the current target (`0x10010fa8`, inferred reading; for the target-acquisition ticket) |
| `ScanDelay n` | `+0x270` (default -1) | ticks between target scans: the scan countdown `unit + 0xc6` (decremented every tick, `0x10017020`) is raised to at least `n + rand(0..15)`, or `2n + rand(0..31)` in a case that sets bit 7 of `unit + 0x1a` (`0x100113a4`, `0x100113f0`; also `0x10016524`) | |
| `Mech 0/1` | `+0x274` | the unit can be repaired by a repair vehicle (inferred) | `0x10032ef0` |
| `Reload1 a b` | `+0x284` = a (int), `+0x278` = trunc(b * 256 + 0.5) | resupply of `Ammo1` from a supply vehicle: `a` is the supplier's stock needed (compared with the supplier's `+0x39`), `b` a rate in 1/256 (inferred). Not a rate of fire | `0x10032f1c`, `0x1001b08f`, `0x10024d19`, `0x1002b851` |
| `Reload2 a b` | `+0x288`, `+0x27c` | same for `Ammo2` | `0x10032f4a` and same functions |
| `Repair a b` | `+0x28c`, `+0x280` | same for hit points (needs `Mech`) | `0x10032ee8` |
| `Expa_LooseHP a,b` | `+0x290` = trunc(a * 256 + 0.5), `+0x294` = trunc(b * 256 + 0.5) - that | experience gained when the unit loses hit points: `max(1, value(+0x3d) * hp_lost >> 8)`, raw `+0x3d` (observed `0x10032d43`, in the hit handler `0x10032c40`) | |
| `CanBeCrushed 0/1` | `+0x298` | can be run over | |
| `CanAttackPoint n` | `+0x29c` (default 0) | non-zero allows attacking a ground point (tank unit vtable `+0x28`, `0x100108b0`, returns early when 0; inferred purpose). 44 tanks have 1, one has 2 | |
| `AlarmRange a,b` | `+0x2a0` = a, `+0x2a4` = b - a (default 5, 1) | radius in **cells**. After loading, the gun vehicle adds `shot1_Range a >> 5` to `a` (`0x10042de0`), so the main gun's range in cells is added. Used as a leash: a unit attacked by an enemy closer than it reacts against the attacker (`0x1000f871`, order type `0xb`); an attack order stops when the unit is farther than it from where the order started (`0x100167df`, order `0xc`) (inferred reading) | |
| `Radius n` | `+0x2ac` | unit radius in pixels: subtracted from the distance between the unit and an impact (`0x1002cd16`, damage path) | `0x10024420`, `0x1002ae30`, `0x1002e780`, `0x1002f5a0`, `0x10070810` |
| `Armor hit f1,f2 s1,s2 r1,r2` | `+0xf0 + 0x18 * hit + 8 * face`: `256 - v1`, then `(256 - v2) - (256 - v1)`, both clamped to 0..256 (`0x10046ac0`); default 256, 0 (no armour) for all 16 hit types | see "Armour" | `0x1003f450`, `0x10033550` |
| `dirBoomType`, `radBoomType` | `+0x2cc`, `+0x2d0` (index in `BOOMTYPES`) | explosion type shown on the unit for a direct hit / an area hit (`0x1003f4a3`, `0x1003f601`; for the visuals ticket) | |
| `DieExplosion name` | `+0x2f4` | effect when destroyed | `0x100336c1` |
| `HellTimeFactor f` | `+0x2d4` (float, default 1.0) | burning time factor (inferred from the name) | `0x1003cf2e` |
| `Selector n` | `+0xb4` | selection marker size (UI) | |
| `Camouflage`, `SelType`, `Score*`, `crew_*`, sounds | | not combat rules (crew leaving a wreck is out of scope) | |

## Tank and gun-vehicle fields

| Key | Stored | Meaning and units | Readers |
|---|---|---|---|
| `GunTurnDelay n` | `type + 0xc9e` | turret: every tick `unit + 0xd0` counts down; at 0 the turret heading `unit + 0xce` steps 1/32 turn towards `unit + 0xcf` (the hull heading `+0x63` when `0xff`), the shorter way, and the counter is reset to `n`. Ticks per 1/32 turn (observed `0x10010b20`-`0x10010b9e`) | |
| `GunShotWait n` | `+0xca2` | after a main gun shot the turret counter `unit + 0xd0` is raised to at least `n`: ticks the turret stays still (`0x1000fe24`). Also sets `unit + 0xcd = 0x20` after a main gun shot and `unit + 0xd1 = 0x20` after a machine gun shot, both counting down per tick (purpose not read; probably the shot's visual) | |
| `DaviRange a,b` | `+0xc96`, `+0xc9a`; `auto` = -999, replaced after loading by `MoveSpeed * 8 / 256` (both values, `0x10042daa`) | maximum distance in cells at which the tank goes to run over a target (`0x1001694a`; Russian "davit'", to crush). Infantry only, out of scope | |

The movement keys (`MoveSpeed`, `TurnDelay`, `MoveDamageHP`, `BackMoveSpeed`, ...) are in
`docs/movement.md`. `MoveDamageHP` is the combat link: below it the tank is immobilised (`unit + 0xb3`,
`0x1002dd30`).

## Weapon records `shot1_*` and `shot2_*` (parser `0x10047e90`, defaults `0x10047e10`)

The key is built as `shot%i_<field>` (or `shot_<field>` for index 0, `0x100b0da8`, `0x100b0da0`). Offsets
are within the 0x93-byte record: add `0xb70` for `shot1`, `0xc03` for `shot2`.

| Field | Record | Meaning and units | Readers |
|---|---|---|---|
| `ID name` | `+0x00` pointer to the shot type from `shots/SHOTS` (`invis tank_expl`...) | projectile class and its hit type (`HitID` in the shot file, kept at shot type `+0x1c`, index in `HITSNAMES`) | |
| `Animation`, `Animat`, `Sound`, `StartBurstSound`, `EndBurstSound` | `+0x04`, `+0x08` (with x, y offsets at `+0x0e`, `+0x10`), `+0x14`, `+0x21`, `+0x2e` | visuals and sounds (`0x10001000`); visuals ticket | |
| `Delay a,b` | `+0x3b` = a, `+0x3f` = **always 0** (the parser subtracts `a` from itself, `0x10048194`-`0x100481a2`) | passed to the shot type's create method (`0x10001730`); meaning not read. No tank uses it | |
| `Damage a,b` | `+0x43`, `+0x47` | hit points carried by the projectile (`projectile + 0x10`, `0x10001730`), before armour | also the target estimate `0x10033550` |
| `Range a,b` | `+0x4b`, `+0x4f` | maximum range in pixels: the weapon may fire only when the octagonal distance to the target is at most this (`0x100102fb`, `0x10010c70`, ...); also passed to the projectile to scale its scatter (`0x100341c2`) | `0x100101f0`, `0x10010550`, `0x10010c70`, `0x10011f50`, `0x10012250`, `0x100125f0`, `0x10012970`, `0x10015da0` |
| `DeadZone a,b` | `+0x53`, `+0x57` | minimum range in pixels: below it the weapon is not used (`0x10010367`) | same functions |
| `Speed a,b` | `+0x5b`, `+0x5f` | projectile speed in pixels per tick: flight time = distance / speed, at most 254 ticks (`0x10041bf4`, `invis` class) | `0x10034070` |
| `Accuracy a,b` | `+0x63`, `+0x67` | scatter: the aim point moves by a random angle and a random length up to `accuracy * distance / range + accuracy / 8` pixels (`0x10041b3c`-`0x10041bed`). Smaller is better (22 green, 12 perfect for the 76 mm) | `0x10034070` |
| `UseAmmo slot n` | `+0x6b` = slot - 1, `+0x6f` = n; `none` or slot 0 gives 0 and `0x7fff` (also the default) | the counter `unit + 0x39 + 2 * slot` must hold at least `n`, and `n` is removed per shot (`0x10001730`). A weapon with no `UseAmmo` therefore never fires (inferred). `1 0` (two tanks) fires without using ammunition | `0x10010272`, `0x10016b43`, `0x10016f33`, `0x10016fa3` |
| `Expa a,b` | `+0x73`, `+0x77` | experience added to the shooter's `+0x3d` per shot, interpolated on the raw `+0x3d` (`0x10001730` -> vtable `+0x1d8` `0x10032e90`) | |
| `ReloadTime a,b` | `+0x7b`, `+0x7f` | ticks between shots. The weapon state is 3 bytes in the unit, main gun `unit + 0xc0`, machine gun `unit + 0xc3`: u16 countdown, u8 burst count. Fire sets the countdown (`0x10001000`), each tick decrements it (`0x10017020`). At creation the countdown is a random fraction of one reload: `rand(0..1023) * reload >> 10` (`0x10017252`, `0x100172f2`). Also used to rate each weapon's damage per tick when choosing which one to fire (`0x10016de4`, `0x10016e4b`) | |
| `BurstShots a,b` | `+0x83`, `+0x87` | shots per burst; 0 = no bursts. A burst counts its shots in the state's byte; when it reaches the value, the countdown takes `BurstReloadTime` instead of `ReloadTime` (`0x10001000`) | |
| `BurstReloadTime a,b` | `+0x8b`, `+0x8f` | ticks after a full burst | `0x10001000` |

Three engine-level facts for the firing ticket (observed): the fire methods are tank unit vtable `+0x2b0`
`0x1000fdf0` (weapon index in `[0x100c451c]`, record in `[0x100c4528]`, `k` in `[0x100c4520]`, target
in `[0x100c4524]`) -> `0x10001730`; the projectile launch is `0x10016850` -> `0x10034070` -> shot type
vtable `+0x2c` (`invis`, the class of every tank shot in `shots/SHOTS`: `0x10041af0`); weapon choice
between the two targets `unit + 0xb8` / `unit + 0xbc` is `0x10016b30`.

## Armour

`Armor <hit> f1,f2 s1,s2 r1,r2` (data, `HU/MEDIUM.ARMOR`): six numbers per hit type (`HITSNAMES`: air,
explosive, fire, machine, mine, pierce, sniper; up to 16), a green-crew and a perfect-crew value for the
**front, side and rear**. Each is protection in 1/256; the game keeps `256 - v`, the share of damage
that passes (`0x10046ac0`). The `-Armor: front=47 side=47...` lines in the unit files are comments; the
real figures come from the included `*.ARMOR` file. Example, T-34 (`medium.armor`) against `explosive`
(the hit type of `tank_expl`, the shot of 31 of the 45 tanks with data): front 160 to 184, side 88 to 96,
rear 72 to 80, so 96/256 to 72/256 of a frontal hit's damage gets through.

In the damage function `0x1003f450` (observed; details belong to the damage ticket): the hit type is the
projectile's (shot type `+0x1c`); the face weight comes from `u = (projectile heading - hull heading) & 31`
folded to 0..16: `u = 0` (shot from behind) uses the rear values, `u = 8` the side, `u = 16` the front,
linearly in between (`u / 8`). Each face value is first interpolated with the target's `k`. The result
times the damage, `>> 8`, then shifted by per-player difficulty factors (`0x101ff495`, `0x101ff499`) and,
in single player, halved or doubled for the local side. The turret heading plays no part: the tank passes
its hull heading `+0x63` and face index -1 (`0x1002cd7c`). The target-choice estimate `0x10033550` uses
the front value only.

## Data coverage (the 47 `tank` types)

Expanded with includes, 45 types parse (`_e_t34` and `_r_koenig` are misread, see below). Every one of
them sets `shot1_*` (ID, Damage, Range, DeadZone, Speed, Accuracy, UseAmmo, Expa, ReloadTime), `Armor`
for six hit types, `AttackPref`, `TargetType`, `Sight`, `ScanDelay`, `GunTurnDelay`, `GunShotWait`,
`Health`, `Ammo1`, `Radius`; 30 have a `shot2_*` machine gun (`man_machine`, bursts of 5 to 6) and 3 a
flamethrower (`ognemet`). Typical medium tank (`HU/76`): Damage 275-325, Range 450-510 px, DeadZone 72-56,
Speed 30, Accuracy 22-12, ReloadTime 140-120 ticks (5.6 s to 4.8 s at 25 ticks/s).

## What `crates/formats/src/desc.rs` handles

- Handles: line splitting on blanks and commas, lowercase keys, `#` includes (archive paths, lowercased
  by `sue::normalize`), `-` comment lines, `//` comments, `int` (`atoi`) and `float` (single precision)
  accessors, the quoted `Name`. The client reads `MoveSpeed`, `TurnDelay`, `GunTurnDelay`,
  `IgnoreMines`, `CanMoveBackward`, `Health`, `Ammo1/2`, `MoveDamageHP`, `Sight`, `Selector`, `File`,
  `Name` (`crates/client/src/assets.rs`).
- Missing for combat:
  - `get()` returns the last line of a key. `Armor` and `AttackPref` (and `SayAck`, `Scream`) are one
    line per sub-key, so all but the last are lost; a lookup by (key, first value) is needed.
  - No typed weapon records: `shot1_*`, `shot2_*` (13 numeric fields each, plus `ID`, `UseAmmo` with its
    `none` case).
  - The load-time transforms: `a, b - a` pairs; armour `256 - v` clamped; `Sight` clamped to 1..31;
    `Reload*`/`Repair`/`Expa_LooseHP` float × 256 + 0.5; `Delay` second value ignored; `DaviRange auto`;
    `AlarmRange` plus `shot1_Range >> 5`; base defaults (`AlarmRange` 5,1, `ScanDelay` -1, armour 256,
    `UseAmmo` 0x7fff).
  - The name tables the values index: `HITSNAMES` (with its `rad step typ add` columns, used by the
    damage function), `TARGETTYPES`, `BOOMTYPES`, `shots/SHOTS` and the shot files (`HitID`).
  - These are inferred to belong in `crates/formats` as plain data (the decoder), with the crew
    interpolation in `crates/sim`.

## Surprises and corrections

1. **`.sue` chunks are always zlib.** `docs/formats.md`, `crates/formats/src/sue.rs` and
   `tools/sue_extract.py` treat a chunk whose stored length equals the expected length as raw. Checked on
   all 24 archives: no such chunk is raw; 31 of them are zlib streams that happen to have the same
   length, and are returned compressed. Among them the tank types `units\_e_t34` and `units\_r_koenig`,
   `H\carabine`, `priceptypes`, the palettes `DOUG.COL` and `plan_87.col` (`units.sue`). The reader
   should always inflate (observed in the data, not in the game's reader).
2. **Tank vtable.** The factory builds `tank` with `0x10042fa0` (type vtable `0x100a8ce8`, unit vtable
   `0x100a8d24` from `0x10042fe0`). `0x100a83f0`, recorded as the tank vtable, belongs to `apc`
   (`0x10042890`, derived from the tank type, hence the passengers). 48 slots differ, among them `+0x1a8`
   update (tank `0x10010b20` directly, no crew step), `+0xc4` idle (`0x1000f6f0`), `+0x298` speed
   (`0x1002ddc0`: forward or `BackMoveSpeed`, interpolated with the veteran-raised `k`, no towing factor).
   `docs/original-code.md` and `docs/movement.md` should be checked against the tank vtable.
3. `shotN_Delay`'s second value is discarded by the parser (`0x10048194`).
4. In the per-tick weapon countdown (`0x10017020`), the machine gun's burst decay compares its countdown
   with the **main gun's** `ReloadTime` (both blocks use `type + 0xb70`, `0x10017156`): an interrupted
   machine gun burst keeps its count until the countdown goes below minus the main gun's reload time.
5. Armour depends on the target crew's `k`, and the target estimate uses the front only.

## Addresses for the other tickets (not followed further)

- Target acquisition: target scoring `0x10010c70` (`AttackPref`, `TargetType`, `Range`, `DeadZone`),
  per-weapon attack checks tank vtable `+0x18` `0x100101f0`, `+0x24` `0x10010550`, `+0x28` `0x100108b0`;
  scan timer `unit + 0xc6`; hit handler `0x10032c40` records the attacker in `unit + 0x16`.
- Firing: `0x10016b30` (weapon choice), `0x1000fdf0`, `0x10001730`, `0x10001000`, `0x10016850`,
  `0x10034070`, `0x10041af0` (`invis` aim and flight time).
- Damage: `0x1002cce0` (impact vs `Radius`), `0x1003f450`, hit type table `0x100c51e0` (32 bytes per
  `HITSNAMES` entry: rad, step, typ, add; `typ` 1 uses `add` as a hit chance in percent, green `% 100`,
  perfect `/ 100 % 100`, interpolated with the shooter's `k`).
- Visuals: `0x10001000` (Animat, sounds), `dirBoomType`/`radBoomType` in `0x1003f450`, `unit + 0xcd`,
  `+0xd1` 32-tick counters after a shot.
