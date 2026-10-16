# From fire decision to impact

Research for issue #4 (map #1, "Tank skirmish"): what happens, tick by tick, between a tank
deciding to fire and its shot landing.

Source: `Game_Dll.dll` from Sudden Strike Gold, read statically: disassembly with
`llvm-objdump`, plus Ghidra pseudocode through REA for the functions Ghidra recognises. The
vtable targets have no Ghidra function, so they were read in the disassembly. Description values
quoted as examples come from the local `desc_common` files (`units/t-34`, `HU/76`,
`HU/shot2.hmg`, `shots/*`, `HITSNAMES`). Every address is in `Game_Dll.dll`.

Conventions: **read** means the code was checked. **Inferred** means a deduction from the code
that has not been confirmed. Nothing here was observed at run time. Map pixels are the
simulation's coordinates: a cell is 32 x 32 map pixels, and a unit's centre pixel is
`cell * 32 + (offset >> 8)` (`0x100334c0`).

## Summary

- A tank has two weapon slots, each with its own target: slot 0, the main gun (`shot1_*`), aims
  with the turret; slot 1, the machine gun (`shot2_*`), is fixed to the hull. The fire check
  runs every tick inside the unit's update (`0x1000fbe0`). A weapon fires when every one of
  these holds: it has ammunition, the target is seen, the target is within range and not inside
  the dead zone, the line of fire is clear, the gun points at the target (the turret exactly, the
  hull within one heading step for the machine gun), and the reload counter has reached 0.
- Firing (`0x10001730`) does all its work at once: it creates a shot record, spends the
  ammunition, adds experience to the crew and sets the reload counter. The shot waits in a
  pending list. The **next** tick's shot pass launches it (`0x10034070`): it re-reads the
  target's position, draws two random numbers to scatter the aim point, and sets a flight time
  of `distance / speed` ticks, between 1 and 254. The flight is a countdown. Tank shells
  (`tank_expl`, `tank_pierce`, `man_machine`) belong to the `invis` class: they have no
  position or sprite in flight and do not interact with anything until they land.
- On landing (`0x10041d80`), the impact point is the scattered aim point. One random byte picks
  whether the ground and scenery choose an explosion. Then every unit in the 5 x 5 cells around
  the impact cell goes through the damage path, whatever its side (inferred from the absence of
  a side test here). A "miss" is a landing point too far from every unit for the damage path to
  register a hit. Damage is out of scope for this note (see issue #5's damage research).
- Scatter depends on the distance, on the weapon's `Accuracy` and `Range`, and on the shooter's
  crew condition (veterans get a boost). The code shows no effect of the shooter's or the
  target's movement, apart from the target moving during the flight: its position is read once,
  at launch, and never led.

## Weapon description fields

The weapon parser `0x10047e90` (called for the tank type at `0x10042d64` / `0x10042d76`) fills a
0x93-byte weapon record: slot 0 at type `+0xb70`, slot 1 at type `+0xc03`. Ranged values `a,b`
are stored as `a` and `b - a`. A value is evaluated for a crew condition `k` (0..`0x8000`) as
`a + ((b - a) * k >> 15)`. This note calls that `lerp(field, k)`.

| Record offset | Key | Use on the fire path |
|---|---|---|
| `+0x00` | `shotN_ID` | shot type object (its vtable drives the shot) |
| `+0x04` | `shotN_Animation` | muzzle animation (visual) |
| `+0x3b` | `shotN_Delay` | ticks the shot waits before launch. **Parser quirk (read):** both stored words come from the first value, so `b - a` is always 0 and the delay is `a` |
| `+0x43` / `+0x47` | `shotN_Damage` | copied into the shot (`shot + 0x10`) at firing |
| `+0x4b` / `+0x4f` | `shotN_Range` | maximum distance; also the scatter divisor |
| `+0x53` / `+0x57` | `shotN_DeadZone` | minimum distance |
| `+0x5b` / `+0x5f` | `shotN_Speed` | map pixels per tick (sets the flight time) |
| `+0x63` / `+0x67` | `shotN_Accuracy` | scatter size (smaller is better) |
| `+0x6b` / `+0x6f` | `shotN_UseAmmo slot count` | ammunition slot `slot - 1` and the rounds spent per shot. With no value, or `none`: slot 0 and 0x7fff rounds, so the weapon never has enough (inferred: disabled) |
| `+0x73` / `+0x77` | `shotN_Expa` | crew experience gained per shot |
| `+0x7b` / `+0x7f` | `shotN_ReloadTime` | ticks between shots |
| `+0x83` / `+0x87` | `shotN_BurstShots` | shots per burst (0 = no bursts) |
| `+0x8b` / `+0x8f` | `shotN_BurstReloadTime` | ticks after a full burst |

Tank-type fields: `GunTurnDelay` `+0xc9e`, `GunShotWait` `+0xca2` (`0x10043030`), `DaviRange`
`+0xc96` / `+0xc9a` (`0x10042cd0`, default -999 = `auto`; used for crushing, see below). The
weapon record's constructor `0x10047e10` zeroes everything except the UseAmmo count (0x7fff).

Shot type fields used here (constructor `0x100416c0`, parser around `0x10041800`):
`CheckStolb` `+0x20` (line-of-fire check on or off), `HitID` `+0x1c` (index into the hit kinds,
`-1` by default), `ExplosionRnd` `+0x28` (stored as `trunc(value * 256 + 0.5)`, default 256),
`DestroyLand` `+0x24`, `dZ` `+0x12c`, and the per-ground explosion slots from `+0x2c`.

Shot classes (factory `0x1004b2c0` area): `invis` (vtable `0x100a82f4`), `pixel` (`0x100a8334`)
and `rocket` (`0x100a8374`). They share creation (`+0x18` `0x10041cf0`), initialisation (`+0x28`
`0x10041a70`), scatter (`+0x2c` `0x10041af0`) and impact (`+0x34` `0x10041d80`). Only the flight
step (`+0x30`) and drawing differ.

### Crew condition used

The shooter's crew condition `+0x3d` (0..`0x8000`). When the unit has a veteran bit (`+0x1a &
0xc000`), the value used becomes `min(0x8000, max(c, 0xccd) + c / 2)` (read in every function
below). The fire path uses this boosted value for range, dead zone, damage, speed, accuracy,
reload and burst values. **Exception (read):** `Expa` is evaluated with the raw crew condition
(`0x100017d2`).

## Per tick: where the firing code runs

The game tick (`0x10090d15`) calls, in order: the sight refresh `0x1004e220`, the **shot pass**
`0x10061990`, the game object's `+0x18`, `0x10068370`, the **unit pass** `0x1003e520`, then
`0x1008e620` and the interface. Shots fired by units in tick T are therefore first processed by
the shot pass of tick T + 1.

Inside the unit pass, a tank's update (`+0x1a8` `0x10014c00` -> `0x10010b20`) runs:

1. `0x10017020`: the target-scan countdown `+0xc6` decreases. The reload counters decrease (see
   "Reload and bursts"). Then the drive step `+0x2a0`, which also turns the hull towards the
   wanted heading `+0x64` (`docs/movement.md`).
2. The "recently fired" timers `+0xcd` (gun) and `+0xd1` (machine gun), set to 32 when the weapon
   fires, decrease by 1. Their purpose is unknown: no reader in the tank's code was found.
3. Turret step (see "Turret").
4. Order dispatch `0x10032950` on `+0x24`. The orders that fire are: 1 idle (`0x10013800` ->
   `0x1000f6f0`), 2 move (`0x1000f9f0`: fire on the move), 3 attack a unit (`0x10016820` ->
   `+0x2b4` `0x1000fe40`), 4 attack a ground point (`0x1000fb10`), and the similar handlers
   `0x1000f900`, `0x1000faa0` (orders 0xc and 0xa), and `0x10011420`, `0x10011620` (shared by
   other orders, not identified).

Every firing handler follows the same pattern: refresh the targets with the scan `+0x2bc`
(`0x10010c70`; it runs only when `+0xc6` is 0, and target choice belongs to the
target-acquisition research), then aim and fire each weapon through `0x100169b0` (or
`0x10016b30` for the attack order). Both call the per-weapon check `+0x2ac` (`0x1000fbe0`) and,
when it says ready, the fire routine `+0x2b0` (`0x1000fdf0`). The ground-point order uses
`+0x2a8` (`0x1000fd10`, slot 0 only) instead of `+0x2ac`.

The firing code works on the current unit's centre pixel `[0x100c4514]` / `[0x100c4518]`, which
the unit pass sets before each update. It passes its results through globals: weapon slot
`[0x100c451c]`, crew value `[0x100c4520]`, target `[0x100c4524]`, weapon record `[0x100c4528]`,
aim point `[0x100c4530]` / `[0x100c4534]`, distance `[0x100c4538]`, wanted heading
`[0x100c453c]` and status flags `[0x100c4540]`.

## The fire check (`0x1000fbe0` -> `0x10001540`)

`0x1000fbe0(slot)` evaluates the crew value and calls `0x10001540(slot, crew, target, weapon)`
with the slot's target (`+0xb8` gun, `+0xbc` machine gun). That function (read):

1. Asks the target for its aim point and distance (target vtable `+0x90`, tank `0x100334c0`).
   The aim point is the target's centre pixel. The distance from the shooter's centre is
   `max(|dx|, |dy|) + min(|dx|, |dy|) / 2` (integer, truncated).
2. Wanted heading = `trunc((atan2(dy, dx) + pi) / pi * 16 + 0.5) & 31`. It uses the game's own
   `atan2` (`0x1005b3f0`) and single-precision pi (`0x100a81a0`, `0x100a81a8` = 32,
   `0x100a81a4` = 0.5); the formula is the heading formula of `docs/movement.md`.
3. Status bits in `[0x100c4540]`:
   - `0x10` no ammunition: ammunition slot `+0x39 + 2 * slot` (u16) below the UseAmmo count;
   - `0x01` target not seen: its cell sight byte `+0x22` has no bit in common with
     `[0x101ff3c8 + player]` (the player's side mask; `docs/original-code.md` lists the same
     table for known mines);
   - `0x04` cannot fire: unseen, or distance greater than `lerp(Range)`. When neither applies
     and the shot type has `CheckStolb`, the line of fire `0x1004dc90` from the shooter's
     centre to the aim point decides;
   - `0x02` too close: distance below `lerp(DeadZone)`;
   - `0x20` ready, when none of `0x02`, `0x04`, `0x10` is set.
4. Back in `0x1000fbe0`, bit `0x08` "not aimed" is set when the turret heading `+0xce` differs
   from the wanted heading (slot 0), or when the hull heading `+0x63` is more than one step from
   it, on the shorter way round (slot 1, the hull-fixed machine gun). `0x20` is kept only if
   `0x08` is clear and the slot's reload counter (`+0xc0 + 3 * slot`, i16) is 0 or less.

The ground-point variant `0x100011b0` (read) takes the point given by the order. It computes the
distance from the shooter's centre the same way. It sets `0x01` when the point's cell is unseen,
but `0x04` only from the range and the line of fire, so **a tank can fire at an unseen ground
point** (read).

## Line of fire (`0x1004dc90`)

Read: the line runs from (x0, y0) to (x1, y1) in map pixels. Its step count is
`n = max(|dx|, |dy|) / 32`, truncated (coordinates in 16.16 after `<< 11`). If `n` is 0 the line
is clear. Otherwise the code walks `n - 1` interior samples: sample `k` lies at
`start + k * delta / n` (per-step increments truncated), and its cell is the sample's `>> 16`.
The line is blocked as soon as one sampled cell has bit 11 (`0x800`) in its flags `0x101054e6`.
The shooter's cell and the target's cell are never tested.

Unlike the sight rays (`0x1004dd40`, which also stop on `0x1000` and use the `0x2000` rule), the
line of fire tests only bit 11. The meaning of bit 11 is still unconfirmed: it is cleared around
houses (`0x100505f7`), which fits "tall scenery" (inferred). In the tank shot files `CheckStolb`
is 1 (`TANK_EXPL`, `TANK_PIERCE`, `MAN_MACHINE`, `OGNEMET`). It is 0 for the indirect weapons
some `tank`-class types carry (`ROCKET_KATYA`, `REACT`, `HEAVYARTS`): those ignore scenery.

## Turret

State: turret heading `+0xce` (0-31, world frame, inferred: it is compared directly with
`atan2` headings), turret target heading `+0xcf` (-1 = follow the hull), turret delay `+0xd0`.

- Each tick (`0x10010b20`, read): if the delay is 0, the turret takes one step (+1 when the
  clockwise difference is 16 or less, otherwise -1, so +1 at exactly 16) towards `+0xcf`, or
  towards the hull heading when `+0xcf` is -1. The delay then becomes `GunTurnDelay` when it is
  smaller. When the delay is not 0, it only decreases by 1. **Rate:** one step every
  `GunTurnDelay + 1` ticks (T-34: `GunTurnDelay 3`, so a full turn takes 128 ticks, about 5.1 s).
- When the hull steps (`+0x288` `0x10010a60`, read): with no turret target, the turret turns with
  the hull. With a target already reached, the turret keeps its world heading and its delay is
  raised to at least `GunTurnDelay`. With a target not yet reached, the turret takes the hull's
  step for free when the hull turns the way the turret needs. Otherwise it keeps its world
  heading and the delay is raised to at least `GunTurnDelay`.
- After a gun shot (`0x1000fdf0`, read) the turret delay is raised to at least `GunShotWait`
  (T-34: 6). The machine gun does not touch the turret.
- The handlers write `+0xcf` every tick from the gun's wanted heading. It is the heading to the
  target when the target is seen, otherwise the value the handler started with: -1 for the idle
  and attack-unit handlers (`0x1000f78d`, `0x1000fe7b`). The move handler (`0x1000fa3d`) writes
  the computed heading whether or not the target is seen, and -1 when the gun has no target.

## Hull turning for the weapons

The hull is turned only by the drive step, at `TurnDelay` rate, towards `+0x64`
(`docs/movement.md`). The firing handlers write `+0x64` only while the tank is not moving (`+0x1a`
bit 2 clear):

- Idle (`0x1000f7af`, read): if the machine gun has a usable target, `+0x64` = the machine gun's
  wanted heading. Otherwise, if the gun sees its target, `+0x64` = the gun's wanted heading: the
  whole tank turns to face it. Otherwise `+0x64` is left as it is.
- Attack a unit (`0x1000fe40`, read): the same choice, made from the bits returned by
  `0x10016b30`.
- Move (`0x1000f9f0`): the handler does not touch `+0x64`. The turret aims and both weapons fire
  on the move; the machine gun fires only when the path happens to point the hull within one step
  of its target.

Inferred: there is no rule that turns the hull because the turret "cannot" reach the target. Any
turret reaches every heading. The hull turns towards the target for the machine gun, and for the
gun when idle, and on a hull step the turret either rides along or keeps its world heading.

## Weapon against target

Read in `0x100169b0` / `0x10016b30`: the gun (slot 0) is tried whenever it has a target and
ammunition. The machine gun (slot 1) is tried only when its target's `+0xb0` (tank `0x10033550`)
returns at least 2. That value is the target's armour value for the shot's `HitID` (type `+0xf0`
table: 24 bytes per hit kind, the first pair evaluated with the **target's** crew condition)
times `lerp(Damage)` of the weapon, `>> 8`. The meaning of the armour values belongs to the
damage research. Target selection (`AttackPref`, scoring by distance) happens in the scan
`0x10010c70`, which this note does not cover. The scan uses the same `+0xb0 >= 2` test for the
machine gun and the same line of fire for the gun when the shot has `CheckStolb`.

## Firing (`0x1000fdf0` -> `0x10001730`)

`0x10001730` (read) runs on the fire tick, inside the shooter's update:

1. Creates the shot from the shot type (`+0x18` `0x10041cf0`): it takes a record from the free
   list `[0x1029dd78]` (pool `[0x1029dd70]`), **prepends** it to the pending list `[0x1029dd68]`,
   and sets the delay byte `+0x19` = `Delay` and the type `+0x4`. With an empty pool the weapon
   does not fire, and nothing else is spent.
2. Initialises it (`+0x28` `0x10041a70`). Shooter id `+0x8`. Aim point `+0xc` / `+0xe` = the aim
   point from the check. Owner player `+0x1a`; `+0x1b` = 0. Start point `+0x14` / `+0x16`: the
   shooter's centre plus the muzzle offset (`+0x4` `0x1000e280`), with screen offsets (ox, oy)
   mapped to map pixels as x + ox / 2 + oy, y - ox / 2 + oy (inferred to be the inverse of the
   isometric projection). Gun muzzle = the `.hot` point for the hull heading (type `+0x1126`)
   plus the point for the turret heading (`+0x1226`). Machine gun = the point for the hull
   heading at `+0x1326`, or (0, 0) when that point is the marker `0x7fffffff`.
3. Damage `+0x10` = `lerp(Damage)`. Target id `+0xa` = the target's id (0 for a ground point).
4. Spends `UseAmmo count` rounds from the ammunition slot.
5. Adds `lerp(Expa)` (raw crew condition) to the crew condition, capped at `0x8000` (`+0x1d8`
   `0x10032e90`). T-34 gun: 75 at crew 0, 1 at full crew.
6. `0x10001000`: sound, muzzle animation (uses the **second** generator `[0x1029912c]`, not the
   game's), and the reload counter (next section).
7. `0x1000fdf0` then sets the "recently fired" timer (`+0xcd` = 32 for the gun, `+0xd1` = 32 for
   the machine gun) and, for the gun, the `GunShotWait` turret hold.

The fire routine itself draws nothing from the game's generator.

### Reload and bursts

Per-slot state, 3 bytes from `+0xc0 + 3 * slot`: an i16 reload counter, then a burst count.

- Set on firing (`0x10001000`, read). Without bursts, reload = `lerp(ReloadTime)`. With
  `BurstShots`, the burst count rises by 1. Below `lerp(BurstShots)`, reload =
  `lerp(ReloadTime)`. On reaching it, the count resets to 0 and reload =
  `lerp(BurstReloadTime)`.
- Decreased each tick, first thing in the update (`0x10017020`, read). With no burst in progress,
  the counter decreases by 1 down to 0. With a burst in progress (count > 0), the counter keeps
  decreasing below 0. When it reaches `-lerp(ReloadTime)`, the count drops by 1 and the counter
  returns to 0: an interrupted burst recovers one shot for every reload period spent idle.
  **Quirk (read):** both slots use slot 0's `ReloadTime` (type `+0xb70 + 0x7b`) for this
  recovery, so the machine gun's burst recovery runs at the main gun's reload pace.
- Fire allowed when the counter is 0 or less (`0x1000fcdc`).
- At creation (`0x100171e0` -> around `0x10017263`), each counter starts at a random fraction of
  `lerp(ReloadTime)`: `(rnd & 0x3ff) * lerp(ReloadTime) >> 10`. That is one draw per slot, slot 0
  first, and the burst counts start at 0.

T-34 numbers: gun reload 140 ticks at crew 0 and 120 at full crew (4.8 s). The machine gun fires
bursts of 5 to 6 shots, 4 to 3 ticks apart, then waits 40 to 30 ticks.

## Ground point and attack-unit specifics

- Ground point (order 4, `0x1000fb10`, read): with no ammunition the order ends. While the point
  is out of reach (range or line of fire), the tank drives towards its cell (`0x1002d100`); if
  that fails (drive status 3), the order ends. Once the point is in reach, the tank stops. If the
  point is inside the dead zone, the order ends. When the gun is ready it fires, and **the order
  ends when the burst count is 0 after the shot**, so one shot for a gun without bursts (read).
- Attack a unit (order 3, `0x1000fe40`, read). When both weapons have nothing usable, and the
  target is within `lerp(DaviRange)` and the tank may enter the target's cell (`0x10016890`), the
  tank drives at it: inferred to be crushing ("davit"). Otherwise the order ends. When the gun
  cannot fire (unseen, out of range, line blocked), the tank chases the target
  (`0x1002d180(2, target)`). Starting a chase draws the generator once, with a 1 in 4 chance of a
  voice reaction (`+0x1c8(1)`).

## Shot pass and flight (`0x10061990`)

Read: the shot pass runs once per tick, before the units, in two loops over singly linked lists,
each walked from its head. Insertion always prepends.

1. **Pending list** `[0x1029dd68]` (newest shot first). For each shot, `+0x1c` (`0x10041cd0`)
   decrements a non-zero delay and keeps the shot pending. At delay 0 the shot leaves the list and
   is **launched** (`+0x20` `0x10041d30`):
   - shooter id 0 (shooter gone): the shot is discarded;
   - shooter id `0xffff`: a special path (`0x10023420`, not read);
   - otherwise the shooter's `+0x220` (tank `0x10016850`) picks the weapon record whose shot type
     matches and calls `0x10034070(shot, record)`.

   `0x10034070` (read): if the shot still has a target id, the aim point becomes the target's
   **current** centre, as seen from the shooter's current centre (target `+0x90`). Then the
   scatter `+0x2c` is called with `lerp(Speed)`, `lerp(Accuracy)`, `lerp(Range)` and 0, using
   the shooter's crew value **at launch**. A launched shot is prepended to the flying list
   `[0x1029dd6c]`; a discarded one returns to the free list.
2. **Flying list** `[0x1029dd6c]`. Each shot's `+0x30` advances its flight (`invis` and `pixel`:
   `0x10041cb0` increments the counter `+0x12` and keeps the shot while it is below the end
   `+0x13`). When the flight ends, the impact `+0x34` runs and the record returns to the free
   list. A shot launched this tick is already in the list, so it takes its first flight step in
   the same pass.
3. `0x100617b0` (not read).

Since `Delay` is 0 in every tank file, a shot fired in tick T launches in tick T + 1 and lands in
tick T + `end` (`end` >= 1, see below). T-34 gun at 450 px with speed 30: `end` = 15, so the shell
lands 15 ticks (0.6 s) after the fire tick.

**Quirk (inferred from the order):** the launch needs the shooter to be alive at the start of tick
T + 1. A tank destroyed later in tick T, after it fired, loses its shot. The functions that clear
a dead or replaced unit's id from the shots are `0x10061ac0` (callers `0x1003edf0`, `0x1003ee76`,
`0x1003f22a`) and `0x10061a50` (`0x1003ed0c`, id swap).

### Scatter and flight time (`0x10041af0`)

Read, with S = speed, A = accuracy, R = range, all evaluated as above:

1. `d` = distance from the shot's start point (the fire-time muzzle) to the launch-time aim point,
   `max + min / 2` (integer).
2. Spread `s` = `d * A / R + A / 8` (integer divisions, truncated), or `A` when R is 0.
3. Two draws from the game's generator: `r1` then `r2`, each `(state >> 16) & 0x7fff`. Angle =
   `r1 * pi * (1 / 16384)` (single-precision pi `0x100a82ac`, `0x100a82f0` = 2^-14, product in
   double): uniform over a full turn. Radius = `(r2 * s) >> 15`, uniform in [0, s).
4. The aim point moves by `trunc(sin(angle) * radius)` in x and `trunc(cos(angle) * radius)` in y.
   The game's own `sin` (`0x1005b1e0`) is a 10-term Taylor series after reduction to [0, pi/2],
   with 1/(2pi) = `0x100abed8` and pi = 3.1415926535. `cos` (`0x1005b2d0`) is `sin(x + pi/2)`. The
   radius is held as a single-precision float.
5. Flight: counter `+0x12` = 0 and end `+0x13` = `d / S`, kept between 1 and 254 (the computation
   is `0 + d / S` clamped to [`0 + 1`, 254]). The heading of flight `+0x18` is computed from the
   start point to the scattered point, with the same heading formula (used for the explosion's
   orientation).

T-34 gun, full crew (A = 12, R = 510): the spread is 8 px at 300 px and 13 px at maximum range.
At crew 0 (A = 22, R = 450) it is 24 px at maximum range. The shooter's and target's movement do
not appear in these formulas. A target that moves during the flight is simply no longer where
the shot lands.

### Path in flight

- `invis` (all tank shells and the machine gun): no position or drawing in flight (its drawing
  slots are empty, `0x10040120`). The shot is a countdown between two fixed points.
- `pixel`: the same flight, drawn as a pixel (`0x10042080`, `0x10042170`, not read).
- `rocket` (`0x100423e0`, read): the same counter and end. Each step it computes the point at
  fraction `counter / end` on the straight segment, with a parabolic height when `+0x130` (from
  `UseH`) is set, and spawns trail animations there, drawing the second generator. Nothing in its
  flight touches the simulation (inferred: no unit or cell test is made).

## Impact (`0x10041d80`)

Read, at the scattered aim point, cell `(x >> 5, y >> 5)`:

1. Off the map: nothing happens (no draw).
2. One draw from the game's generator. When byte 2 of the new state (`(state >> 16) & 0xff`) is
   below `ExplosionRnd * 256` (always true at the default 256; `OGNEMET` sets 0.1),
   `0x1004c740` picks the explosion slot. It takes the largest of: the cell's ground kind (flags
   bits 8-10), `0x1004fca0` (not read), `0x1004d860` for each cell of the 5 x 5 square, and the
   reaction of every map object in those cells (`0x10052620` -> object class vtable `+0x18`).
   That object path is where scenery reacts: it is the cut point for "scenery takes no damage".
   Whether those class handlers draw the generator is unconfirmed (there are generator references
   in the object classes' code, around `0x10053300`).
3. Units, by hit kind `typ` of the shot's `HitID` (`HITSNAMES`, table `0x100c51d0`, 32 bytes per
   kind: name, `+0x10` rad, `+0x14` step, `+0x18` typ, `+0x1c` add; loader `0x1004a2a0`):
   - `air` (3): `0x1008e6e0` (not read);
   - `one` (1, `sniper`): only the target unit, when the shot still has one;
   - `all` (0, `explosive`) and `one2x` (2, `pierce`): every unit in each cell of the 5 x 5
     square around the impact cell (clamped to the map), row by row from the top, x increasing,
     each cell's units in list order.

   Each unit gets its `+0x1d0` (tank `0x1002cce0`). It measures the distance from the impact
   point to its centre, minus the type's radius `+0x2ac` (`Radius 17` for medium tanks), and
   calls the damage function `0x1003f450`. Read there: a distance of `rad` or more has no effect,
   and a negative one counts as 0. The rest of that function belongs to the damage research. The
   largest explosion slot returned replaces the ground's.
4. When a slot results, `0x10048ff0` spawns the explosion at the point, with the flight heading
   and the owner player.

**Where a missed shot lands:** at the scattered point, and nowhere else. "Missing" happens in
the damage function when no unit is close enough: `explosive` and `pierce` both have `rad` 32
here, against a distance already reduced by the target's radius. Inferred from the numbers above:
a tank gun aimed at a stationary tank lands within about 13 to 24 px of its centre, so it almost
always reaches the damage path. Real misses come from the target moving during the flight, or
from units standing near the landing point (any side, including friendly units: no side test was
seen before `0x1003f450`).

## Random draws, in tick order

The game's generator `[0x10299130]` (`x * 0x41c64e6d + 0x3039`) on the fire-to-impact path:

1. **Shot pass** (before the units), pending list from the newest shot: **2 draws per launched
   shot** (scatter angle, then radius). There is no draw for a discarded or delayed shot.
2. **Shot pass**, flying list from its head: **1 draw per landing shot on the map** (explosion
   choice), then any draws in the scenery reactions (unconfirmed), then any draws in the damage
   function for each unit in the 5 x 5 cells (damage research; one is visible in `0x1003f578`,
   for the `one` kind).
3. **Unit pass**, for each tank in list order, inside its update:
   - drive step draws (`docs/movement.md`);
   - when the scan runs (`+0xc6` = 0): **1 draw** with 1 in 8 chance of a voice reaction, made
     only when neither weapon has ammunition (`0x10010e44`). Then **1 draw** for the next scan
     delay (`0x1001138c` / `0x100113d8`: `+0xc6` = type `+0x270` plus `rnd & 15`, or twice
     `+0x270` plus `rnd & 31`, depending on a distance test; target acquisition research);
   - attack-unit handler, when it starts a chase: **1 draw** (1 in 4 voice reaction,
     `0x1000fec4` / `0x1000ff0c` / `0x1000ffa1`);
   - firing: **no draw**.
4. At unit creation (`0x100171e0`): **2 draws**, the reload counters of slot 0 then slot 1.

The muzzle animations, rocket trails and sounds use the second generator `[0x1029912c]` and can
be left out of the simulation.

## Open questions

- The meaning of cell flag bit 11, the only line-of-fire blocker, and which map objects set it.
- Whether the object-class reactions on impact (`0x10052620` -> class `+0x18`) and `0x1004fca0`,
  `0x1004d860` draw the game's generator. If they do, the skirmish must replay them even though
  scenery takes no damage.
- `0x100617b0` at the end of the shot pass, `0x10023420` (shooter id `0xffff`) and the `pixel`
  drawing were not read.
- The purpose of the 32-tick timers `+0xcd` / `+0xd1`.
- The orders handled by `0x1000f900` (0xc), `0x1000faa0` (0xa), `0x10011420` and `0x10011620`.
- The `+0x1c8` voice reactions are gated by the game's generator. They must keep their draws even
  without sound.

## Notes for the other tickets

- Target acquisition: the scan `0x10010c70` (vtable `+0x2bc`; args range bonus 0x80 or 0xc0,
  angle 0x30, flag) chooses `+0xb8` / `+0xbc`. It scores the gun by `AttackPref` (type
  `+0xd0 + 4 * target type`) and distance, and gates the machine gun with `+0xb0 >= 2`. The
  rescan delay is `+0xc6`. The idle handler `0x1000f6f0` switches to order 0xb when attacked by
  `+0x16` within `AlarmRange` (type `+0x2a0` / `+0x2a4`).
- Damage: `0x1003f450` (args: unit, distance minus radius, ..., hull heading, armour table type
  `+0xf0`). Hit kinds are at `0x100c51d0`. Expected damage is computed by `+0xb0` `0x10033550`.
- Description fields: weapon record layout above; tank type `GunTurnDelay` `+0xc9e`,
  `GunShotWait` `+0xca2`, `DaviRange` `+0xc96`; shot type `CheckStolb` `+0x20`, `HitID` `+0x1c`,
  `ExplosionRnd` `+0x28`, `dZ` `+0x12c`.
- Visuals: muzzle animation `0x10001160` (`shotN_Animation`, second generator), explosion spawn
  `0x10048ff0` with the flight heading `+0x18`; the `invis` class has no flight drawing.
