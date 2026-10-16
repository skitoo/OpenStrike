# Tank damage, damage states and destruction

Research for issue #5 (map #1, Tank skirmish). Source: `Game_Dll.dll` of Sudden Strike Gold, read with
Ghidra 12.1.4 (REA) and `llvm-objdump`; data from `desc_common.sue` (`HITSNAMES`, `HU/*.ARMOR`, `units/*`,
`shots/*`, `explosions/*`). The manual (`manuel SS1.pdf`, pages on life, experience and damage) was used only
as a guide. Everything below was read in the code unless marked **inferred** or **unconfirmed**. Nothing was
observed by running the game.

Lengths are in the game's map units: a cell is 32 units, a unit's point is `cell * 32 + (offset >> 8)`
(`0x100334c0`). Crew condition `k` is the unit's `+0x3d` (0 to `0x8000`); wherever the code reads it, it first
applies the officer boost when bits `0xc000` of `+0x1a` are set (`max(k, 0xccd) + k / 2`, at most `0x8000`; for
example `0x1002cd22`). The rest of this note writes `k` for that boosted value.

## Summary

- A shell's impact is one virtual call on the shot type (slot `+0x34`, `0x10041d80`). It calls every unit
  standing in the 5 x 5 cells around the impact cell. Each unit works out its own damage: falloff with distance
  to its edge, armour by face, then difficulty shifts. The unit then applies it to its hit points (`+0x37`).
- **Impact face.** The face comes from the shot's direction byte against the target's **hull** heading, never
  the turret. The three armour faces (front, side, rear) are interpolated linearly over the angle in steps of
  1/32 turn. Each face is a min..max pair, and the target's crew condition picks the point inside it.
- **Armour.** Armour scales damage; it never stops it. The damage that passes is `(256 - armour) / 256`.
  There is no penetration test.
- **Damage kinds.** `HITSNAMES` gives each kind's radius, step and type. "Double damage on a direct hit" is
  really a halving: armour-piercing (`one2x`) damage is halved for every unit except the one the shot was aimed
  at. Explosive damage is not halved.
- **Slow regain and slow loss.** Every 64 ticks a tank below 1/4 of its health loses 1 HP. An idle tank
  between 1/4 and 3/4 regains 1 HP. The manual's "close to the maximum" is really the band below 3/4.
- **Experience.** Experience is crew condition. A hit raises the victim's crew condition by
  `Expa_LooseHP x damage`. Crew condition then improves armour (inside each face's range), speed, sight, turn
  delay and, as a shooter, damage and accuracy.
- **Damage states.** The only state is "immobilised", entered and left on the same path as hit points
  (HP below `MoveDamageHP`). It changes the hull sprites. There is no "turret out" state for tanks.
- **Destruction.** A destroyed tank stays a unit with its id, flagged dead and immobilised, its HP reset to
  full. That HP then burns down (1 HP with probability 1/4 per tick, half of any further damage). At zero the
  unit is removed outright and its id freed. **Tanks leave no wreck object.** Its `DieExplosion` fires a
  secondary 100-damage explosive shot. Its crew may bail out as infantry (out of scope).
- **Random generator.** The shared generator `0x10299130` is drawn on several damage paths, including the
  speech and voice reactions. The simulation has to make those draws even without sound.

## 1. Where an impact is processed

Order within a tick (`0x10090d59`): sight refresh `0x1004e220`, then **shots** `0x10061990`, then the game-mode
object `[0x10388450]` `+0x18`, AI teams `0x10068370`, then the units `0x1003e520`. All damage is therefore
applied before the units update in the same tick.

`0x10061990` walks two lists of shot records:

- `[0x1029dd68]`, waiting shots: method `+0x1c` counts down, `+0x20` launches the shot into the second list.
- `[0x1029dd6c]`, flying shots: method `+0x30` flies; when it returns 0, method `+0x34` is the impact.
- Finished records go to the free list `[0x1029dd78]`.
- New shots are prepended to `[0x1029dd68]` (`0x10041d08`). **Inferred:** a shot created during an impact (a
  `DieExplosion` add-shot, say) starts its life on the next tick.

Fields of a shot record read by the damage code (offsets from the record):

| Offset | What |
|---|---|
| `+0x04` | shot type (its `+0x1c` is the hit kind index into `HITSNAMES`) |
| `+0x08` | shooter unit id (u16; 0 or `0xffff` for none) |
| `+0x0a` | aimed unit id (u16; 0 when attacking ground or for secondary shots) |
| `+0x0c`, `+0x0e` | impact point (u16 map units) |
| `+0x10` | damage (u16) |
| `+0x18` | direction byte, compared `& 31` with headings. **Inferred:** the flight heading in 1/32 turn (it is written by the launch, which belongs to the firing ticket) |
| `+0x1a` | player of the shooter (12 = none) |

When a shot is fired (`0x10001730`), its damage is `Damage min + (max - min) * k_shooter >> 15`, and `+0x0a` is
the target unit's id or 0. Firing also gives the shooter `Expa` experience (`0x100017e7`).

Impact, shot-type method `+0x34` = `0x10041d80` (pointer at `0x100a8328` in the vtable `0x100a82f4`; the same
code serves `0x100a8334` and others):

1. Impact cell = point >> 5. Outside the map, nothing happens.
2. **Random draw (always one):** `(rng >> 16) & 0xff < shot type +0x28` picks a scenery object at the impact
   through `0x1004c740`. Otherwise no object is picked. **Inferred:** a chance to hit scenery. Scenery damage
   is out of scope, but the draw has to be kept.
3. The hit kind's type (`HITSNAMES` column `typ`) chooses the targets:
   - `air` (3): only planes (`0x1008e6e0`). Tanks are never touched.
   - `one` (1): only the aimed unit `+0x0a`.
   - otherwise: every unit in cells `(cx-2..cx+2, cy-2..cy+2)`, clamped to the map, rows y outer, x inner, each
     cell's list through `unit + 0x0e`. Each unit's `+0x1d0` gets the record (section 2).
4. Each `+0x1d0` returns a "boom type" index (`BOOMTYPES`). The impact keeps the largest one, starting from the
   scenery object's index. If the shot type has an `Explosion` entry for that boom type (table `+0x2c`), it
   spawns that explosion at the impact (`0x10048ff0`, with the record's direction and player). This choice is
   visual (for example `tank_dir` / `tank_rad` -> `tank`), but an explosion can carry add-shots (section 6).
   **For the visuals ticket.**

Cut: step 2 (scenery) and the `air` branch are out of scope. Keep the random draw of step 2 and treat the object
as absent when no scenery is modelled. That changes the boom type only, never unit damage, except for the
`param_11` flag described in section 2.

## 2. Damage computed per unit

`+0x1d0` = `0x10032c40` (base unit, shared by the tank vtable `0x100a83f0`) calls `+0x1cc` = `0x1002cce0`
(vehicles). That one computes:

- the unit's point and its distance to the impact (`0x100334c0`: `max(|dx|, |dy|) + min(|dx|, |dy|) / 2`)
  **minus the type's `Radius`** (`+0x2ac`, 14 for the T-26). The distance is measured to the unit's edge, not
  its centre;
- crew `k` of the target (boosted);
- then `0x1003f450(unit, x, y, dist, hull heading +0x63, k, armour table type +0xf0, face -1, record, &boom,
  flag)`.

`0x1003f450`, step by step (`kind` = record type `+0x1c`, hit table `0x100c51d0`, 32-byte entries: name,
`+0x10` rad, `+0x14` step, `+0x18` typ, `+0x1c` add; loaded by `0x1004a230` from `HITSNAMES`):

1. `typ == 3` (air): no damage (returns -1).
2. Distance: below 0 counts as 0. At `rad` or more, no damage (-1). At 0, the boom type becomes the target's
   `radBoomType` (`+0x2d0`).
3. `typ == 1` (`one`, sniper): the hit chance is `add % 100` plus
   `((add / 100 % 100) - add % 100) * k_shooter >> 15`. That is 75 % to 90 % for `9075`, and the low value
   when the shooter is gone. **Random draw:** a miss (-1) when the chance is below `((rng >> 16 & 0x7fff) *
   100) >> 15`. Damage is the full record damage, with no falloff. Infantry only: out of scope.
4. Every other type: falloff `D' = D - ((D * dist / step) * step) / rad` (integer divisions). The `step`
   column quantises the product `D * dist`; it is not a distance step. Then:
   - if the unit is the aimed one (`unit id == record +0x0a`) and `add != 0`, and `dist == 0`: the boom type
     becomes `dirBoomType` (`+0x2cc`);
   - if the unit is the aimed one and `add == 0` (explosive): nothing more. An `atan2` of the impact offset is
     computed and thrown away (`0x1003f647`, result popped); it has no effect and draws nothing;
   - if the unit is **not** the aimed one: the same thrown-away `atan2`, and when `typ == 2` (`one2x`, pierce)
     the damage is halved (`sar`, `0x1003f6a6`).

   That halving is the manual's "double damage on a direct hit". The direct hit is decided by identity (the
   unit the shot was aimed at, still inside `rad` of its edge), not by where the shell lands. A secondary shot
   (`+0x0a = 0`) never counts as aimed. When a unit dies, `0x10061ac0` clears its id from both shot lists, so a
   shell in flight towards a dead unit hits like a ground shot.
5. **Armour** (the tank always passes face -1): the table entry for this hit kind is 3 faces x (pass, range),
   24 bytes. The faces are front, side and rear, in that order (inferred from the interpolation below and the
   `.ARMOR` data, which is strongest first). Each face's pass value is `p + r * k >> 15` (arithmetic shifts), so
   the crew condition picks a point inside the face's range. The angle is `a = (dir - hull heading) & 31`,
   folded to 0..16 (`32 - a` above 16). Then:
   - `a < 8`: `pass = rear + ((side - rear) * a >> 3)`;
   - `a >= 8`: `pass = side + ((front - side) * (a - 8) >> 3)`.

   `a = 0` (the shell travels the way the tank faces) is the rear and `a = 16` the front. Damage = `pass * D'
   >> 8`. With a face index other than -1, the face is read directly; tanks never do this.
6. If `flag` (the scenery object's entry, passed through `+0x1d0`'s second argument) is non-zero, the result
   is 0. The unit still counts as hit (alerts, speech) but takes no damage. **Unconfirmed:** what that entry
   means for scenery.
7. Difficulty: `D <<= slot(shooter player) +0xb1`, then `D >>= slot(target player) +0xad`. Player slots are
   0xb5 bytes from `0x101ff3e8`, and the shooter shift applies only when the player is below 12. Both fields
   are 0 normally; a cheat toggle (`0x10082a1c`) sets both to 10 for the local player. In single player
   (`[0x10388464] == 0`), the setting `[[0x10388594] + 8]` acts as follows:
   - 0: damage to the local player `[0x10387ca0]` halved, damage from the local player doubled;
   - 2: damage to the local player doubled;
   - otherwise: nothing changes.

   **Inferred:** this is the difficulty (easy, normal, hard). The skirmish can use "normal", that is no change.

Armour parser (`Armor <kind> a,b c,d e,f`, `0x10046ac0` from `0x10046b60`): each pair `(a, b)` is stored as
`pass = 256 - a` (clamped to 0..256) and `range = (256 - b) - pass`, also clamped. Armour `a..b` out of 256 is
therefore the share of damage stopped, from `a` (crew 0) to `b` (full crew). Example `HU/LIGHT.ARMOR`, `pierce
64,80 48,64 32,48`: front 64 to 80, side 48 to 64, rear 32 to 48. `HITSNAMES` rows (name rad step typ add):
air 75 1 air 1, explosive 32 1 all 0, fire 40 1 all 1, machine 10 1 all 1, mine 64 1 all 1, pierce 32 8
one2x 1, sniper 8 1 one 9075. These match the manual's table of radii.

Scope: all of this is tank-only except the `one` type (sniper) and the `machine` / `sniper` armour lines that
infantry weapons use. The armour lines are data and cost nothing to keep.

## 3. Applying the damage

`0x10032c40`, once the damage `D >= 0` is known (a negative result is a miss: nothing at all happens):

1. If the record's damage is non-zero, ticks since the last hit = tick - `+0x42`. Above 200 ticks: speech event
   2 (`+0x1c8(2)`, **inferred** "under attack"). Above 300, when the shooter's player is neither the target's
   own nor 12: `+0x1c4(0x80, 0)`, which for the local player's units calls `0x10082630(0x80, x, y)`
   (**inferred:** the minimap's red alert circle, for the visuals ticket). Then `+0x42 = tick`.
2. If the shooter exists, is seen by the target's side (`shooter +0x22 & [0x101ff3c8 + target player]`) and is
   hostile (`[0x101ff390 + 2 * target player]`): `+0x16 = shooter`, `+0x1d = 0`, `+0x1e = D`, and
   `0x1003ef80(target, shooter)` calls `+0x178(target, shooter)` on every unit in the 5 x 5 cells around the
   target (**for the target-acquisition ticket**; `0x100338e0` ages `+0x16` and drops it after 200 ticks).
3. If the record's damage is non-zero: `+0x20 = 0` (a tick counter that otherwise counts up in `0x100338e0`).
4. If `D != 0`:
   - **Alive** (`+0x1a & 1` clear) with HP != 0: experience (section 5), then `0x1003ef50(unit, -D, 0, shooter
     player)`. That sets `[0x100b74f8] = unit` (the "damage from a hit" marker) and calls `+0x1d4`.
   - **Dead**, with `D > 0`: `HP = max(0, HP - (D >> 1))` (the wreck, section 7).

`+0x1d4` for tanks is `0x1002dbe0`. It first calls the base `0x10032de0(delta, arg, player)`:

- `delta > 0`: `HP = min(HP + delta, health)`;
- `delta <= 0`: `+0x41 = player` (the last attacker, unless 12), `HP = max(0, HP + delta)`;
  - HP reaches 0: death, `0x1003edc0(unit, arg, player)` (section 7);
  - otherwise, when `HP < health >> 1` and `-delta > health >> 4`: a **random draw**, and when bit 16 of the new
    state is clear, speech event 3 (**inferred** "close to death").

Then, only when `[0x100b74f8]` is set (any call through `0x1003ef50`):

- `HP >= MoveDamageHP`, alive and immobilised: mobile again (`+0xb3 = 0`, `+0x294` stop and snap,
  `0x1002d720`);
- `HP < MoveDamageHP` and not yet immobilised: the moving bit 2 of `+0x1a` is cleared, `+0xb3 = 1`, and `+0x290`
  (`0x1002d610`) runs. That ends the movement, moves the unit into the cell under its centre and reserves it on
  layer 1 with the direction of its offset from the cell middle (8 when within `0xa00` of it, otherwise the
  octant of `atan2`).

Speech events (`+0x1c8(n)` -> `+0x1c4(n, type +0x9b9)`, `0x10033b80`): while the unit's timer `+0x46 + 4n` is
in the future, **one random draw** brings it forward by `rng >> 16 & 3`. Otherwise a voice is chosen by crew
level (`k >> 13`, at most 3) and played (`0x1006bd50`, sound). Then `0x1003f070` resets the timers of the units
within 12 cells to `tick + delay + (rng * delay >> 16)`, **another random draw**. Sound is out of scope, but these
timers and draws have to be kept for determinism. The callees of the sound engine (`0x1006b950` and others) were
not checked for the shared generator. **Unconfirmed.**

## 4. Slow regain and slow loss

- **Loss:** each tank update (`0x10010b20`, end): when `(id & 63) == (tick & 63)` and `0 < HP < health >> 2`,
  `0x1003ef50(unit, -1, 0, 12)`. This loss can kill.
- **Regain:** the tank's idle handler (`0x10013800` -> `0x1000f6f0`, both branches): on the same 64-tick phase,
  when `health >> 2 <= HP < health - (health >> 2)`, `0x1003ef50(unit, +1, 0, 12)`. It happens only while the
  unit is idle and its target search returned status 3 (`0x1000f8ab`; **inferred:** nothing to shoot), or with bit `0x20` of `+0x1a` set (meaning unknown).
- Both go through the hit marker, so the regain can lift a tank back over `MoveDamageHP` and make it mobile
  again. With the T-26 (300 HP, `MoveDamageHP` 150) the regain band is 75 to 224, so an immobilised T-26 left
  idle repairs itself. **Inferred** from the code, not from play.
- Between `3/4 x health` and full there is no regain, which disagrees with the manual's wording.
- A separate rule heals passengers (`0x10014d71`): every 64 ticks, each passenger record (`+0xdd` count, 0x13
  bytes from `+0xde`, HP at `+0x04` of a record) between 1/4 and 3/4 of its own type's health gains 1. These
  are infantry: out of scope.

## 5. Experience

Experience is the crew condition `+0x3d`. The bar shows it out of 1000.

- **On being hit** (`0x10032d40`): gain `g = (Expa_LooseHP_min + range * k >> 15) * D`. When `g >= 256` (masked
  with `~0xff`) the gain is `g >> 8`, otherwise 1. It is added through `+0x1d8` (`0x10032e90`: `k = min(k +
  gain, 0x8000)`). The parser (`0x100471c6`) stores `trunc(a * 256 + 0.5)` and `trunc(b * 256 + 0.5) - that`.
  For `Expa_LooseHP 64,1`: a crew at 0 gains 64 per HP lost, a full crew 1 per HP lost.
- **On firing:** `Expa` of the shot (`0x100017e7`). **On crushing:** `Expa_Crush` (`0x1002d4d1`). Both belong to
  the firing and movement tickets.
- **Effects read in the code:** armour inside each face's range (section 2); shot damage and accuracy
  (`0x10001730`, `0x1000165b`); speed, turn delay and sight (`docs/movement.md`, `docs/original-code.md`); the
  sniper hit chance; the size of the next experience gain; the voice set. The manual's "takes cover sooner" is
  infantry.
- **Officer boost:** bits `0xc000` of `+0x1a` are set by an officer passenger's aura (`0x1003fd90` from
  `0x10014d2f`, type flag `0x200000`, `OfficerRadius`). They count down one step every 32 ticks
  (`0x1003e545`). Infantry passengers only: out of scope, and without them `k` is the raw crew condition.

## 6. Damage states

- Hit points drive one state only, **immobilised** (`+0xb3`, 0 or 1 for tanks), through `MoveDamageHP` on the
  same path as the damage (section 3, and `0x1002dd30` at placement). An immobilised tank keeps turning its
  turret: the turret code (`0x10010b20`) does not read `+0xb3`. **Inferred**, and probably it keeps firing too;
  the orders that refuse to move read `+0xb3` (for example `0x10010311`). This needs checking by the firing
  ticket.
- **No "turret out" state exists for tanks.** No parsed key other than `MoveDamageHP` gates on hit points (bounded
  search over the parser `0x10046b60` / `0x100479c0` and the writes to `+0xb3`). The writes of 2, 3 and 4 into
  `+0xb3` (`0x10013ac4` and others) are on unloaded soldiers, where the field means something else.
- **Images:** the draw state (`0x1000e4b0`) packs `+0xdb` as turret heading (bits 0-4), hull heading (bits 6-10)
  and a state (bits 11-12): 2 when dead, otherwise `+0xb3 & 3`. The hull pass (`0x1000eeb0`) draws frame table
  `+0xca6[hull]` and then `+0xd26[state * 32 + hull]`. That is the damaged hull when immobilised (**inferred:**
  frames 32-63 of the `.pck`, see `docs/formats.md` "hull variants"). When dead, a single sprite
  `+0xe26[hull]` is drawn with no turret. **For the visuals ticket**: the exact frame numbers come from the
  type's sprite tables.

## 7. Destruction

Death (`0x1003edc0(unit, arg, player)`, from HP reaching 0):

1. `+0x41 = player` (unless 12).
2. Every unit's `+0x1e8` (tanks `0x10017410`: forget it as weapon target `+0xb8` / `+0xbc`, `0x1002db80`).
3. `0x10061ac0`: its id cleared as shooter and target in both shot lists. `0x1005de50`: cleared from queued
   orders. `0x10083800`: cleared from the interface's unit slot `0x103880b8`. Sound manager `0x1006be60`.
4. `+0x1b4` (tanks `0x10014e00`): a towed or attached object `+0x176` released (`0x100265e0`); each passenger slot
   handed to `[0x10388450] +0x20` (infantry, out of scope); then `0x10010b00` (weapon timers `+0xcd`, `+0xd1`
   cleared) -> `0x1002dcb0`:
   - base death `0x10033690(arg, player)`:
     - flags `+0x1a &= 0xc2f7`, then `|= 1` (dead);
     - unless `arg & 4`: the type's `DieExplosion` (`+0x2f4`) spawned at the unit's point (`0x10048ff0`,
       direction 0, killer's player);
     - unless `arg & 8`: die sound (type `+0x2f8`, when audible);
     - unless `arg & 2` (crushing passes 2): **crew bail-out**, a **random draw** whose byte
       `(rng >> 16) & 0xff` must be below `crew_ver` (`+0x2e0`). If so, for each of the `crew_number`
       soldiers: spawn `crew_unit` (`0x1003f810`), then **a draw** whose failure skips the next soldier.
       Quirk: the parser (`0x100475de`) reads the **first** number of `crew_ver` twice, so the range `+0x2e4` is
       always 0 and the threshold is `trunc(min * 256)`: 192 (75 %) for `crew_ver 0.75,1.0`. Infantry: out of
       scope. The draws still have to happen while infantry is absent, for determinism; the cut is to make
       the draws and not spawn anyone;
     - **HP = the type's health** (`+0xb8`);
     - `[0x10388450] +0x1c(unit)` (game-mode object, vtables `0x100ac780` / `0x100ac988`; **inferred**
       end-of-game or score bookkeeping, for the map's end-of-skirmish question);
   - then `+0xb3 = 1` and `+0x290`: the dead tank stays reserved as an immobilised unit on its cell.
5. Selection bit `0x100` of `+0x1a` cleared, `+0x40 = 0`.

The dead tank **keeps its id, its place in the unit list and its cell**. Every tick the unit loop calls
`+0x1ac` instead of `+0x1a8` (`0x1003e5e2`). For tanks that is `0x1002dc60`: a **random draw**; when bit 16 is
set, **a second draw** and HP -= bit 16 of it (so 1 HP with probability 1/4 per tick). Hits on it subtract
`D >> 1` (section 3); its speech and alert side effects still run. When HP <= 0, `0x1003ee50` removes it:

- every unit's `+0x1e8`, the shot and order cleanups again;
- `+0x1a4` (tanks `0x10014b40`: interface rectangles, then `0x1002cdd0`: reservation released and the unit
  taken out of its cell list, `0x10033640`);
- fixes the unit-loop cursors `[0x100b7500]`, `[0x100b74fc]` and `[0x100b74f8]`;
- unlinks it from `[0x100c4508]`, frees its id slot `0x100b7504[id] = 0` and frees the memory. **Inferred:** the
  id can then be reused, since creation takes the first free id from `player * 1024 + 1` (`0x1003e370`).

**No wreck object for tanks.** The `unit`-class wreck ring (`0x10294a88`, adder `0x10056360`) is fed only by a
soldier's death (`0x1001b974`) and by the vehicle class whose `+0x1b4` is `0x10024f00` (vtables around
`0x100a9bc8`, `0x100a9eb4`, `0x100aa1e4`). Those are removed the tick after death by their `+0x1ac` `0x10024e80`,
which unloads up to 2 passengers. **Inferred:** apc, jeep and truck. The nine vtables using `0x1002dc60`
(tanks and the like) never add one. A destroyed tank is drawn by its own dead sprite until its wreck HP burns
out, then vanishes: about `4 x health` ticks on average, 48 s for a T-26. **Unconfirmed** against play.

**Die explosion.** `DieExplosion dead_small` (`explosions/DEAD_SMALL`) carries `AddShot invis`, `AddShotDmg
100`, `AddShotDelay 0 3`, `AddShotAccuracy 1 10`, `AddShotNum 1 1` and `AddShotRnd 0.45`. In `0x10049131`:

- a **draw** decides whether there are add-shots at all (byte `<=` the threshold `+0x41`);
- a **draw** gives their number;
- **3 draws** per shot give angle, delay and distance;
- each shot is an explosive (`invis` = `HitID explosive`) with damage 100, no shooter (`+0x08 = 0xffff`), no aim
  (`+0x0a = 0`) and direction 0.

It damages tanks within 32 of their edge (falloff, face from direction 0 against each hull). It is in scope.
The explosion's animations use the voice generator `0x1029912c` (`0x100490d2`), not the shared one.

## 8. Random generator uses on these paths (shared `0x10299130`)

In tick order:

| Where | Address | Draws |
|---|---|---|
| shot impact, scenery pick | `0x10041dcd` | 1 per impact, always |
| `one` hit chance (sniper) | `0x1003f578` | 1 per unit tested |
| speech event timer (hit after 200 ticks; close to death) | `0x10033bcc`, `0x1003f073` | 1 or more |
| close to death | `0x10032e5f` | 1 per heavy hit under half health |
| crew bail-out | `0x100337c0`, `0x10033872` | 1 + 1 per soldier |
| die explosion add-shots | `0x1004913c` and following | 2 + 3 per add-shot |
| wreck burning (unit loop) | `0x1002dc60` | 1 or 2 per dead tank per tick |

## 9. Scope cuts

| Part | Out-of-scope reach | Cut |
|---|---|---|
| Impact, scenery pick | scenery damage (`0x1004c740`, `DestroyLand`) | keep the draw, no object |
| `one` / `air` kinds | infantry snipers, planes | not used by tank guns (`tank_expl` explosive, `tank_pierce` pierce) |
| Alerts `+0x178`, `+0x16` | target acquisition (its own ticket) | none |
| Speech events | sound | keep the timers and draws, drop the sound |
| Passengers, officer aura, passenger healing | infantry | none in a tank-only skirmish. **Unconfirmed** whether map crew records fill the passenger slots of tanks (`+0x254` -> `+0x258`); check `Exercise 2 Tanks` |
| Crew bail-out | infantry | keep the draws, spawn nobody, record the rule |
| Die explosion | none (explosive add-shot) | in scope |
| Wreck | none: the dead unit itself | in scope |
| `[0x10388450]` notification | game mode, score | for the end-of-skirmish question |

## Open questions

- The direction byte `+0x18` of a shot record and the shot type's `+0x28`, as written at launch (firing ticket).
- What the scenery entry passed as `+0x1d0`'s second argument means (step 6 of section 2).
- Bit `0x20` of `+0x1a` in the idle handler.
- Whether the sound engine's callees use the shared generator.
- What `[[0x10388594] + 8]` is in the menus (difficulty, inferred).
- Whether destroyed tanks vanish in the running game as the code suggests.

## Addresses for the other tickets

- Firing: `0x10001540` (shot setup, angle, accuracy), `0x10001730` (shot creation, damage from crew),
  `0x100017e7` (Expa on firing), shot-type vtables `0x100a82f4`, `0x100a8334` (`+0x18` create, `+0x1c` / `+0x20`
  wait / launch, `+0x28` init, `+0x30` fly, `+0x34` impact `0x10041d80`), shot lists `0x10061990`.
- Acquisition: `0x1003ef80` -> `+0x178` (tank `0x10032640`), attacker memory `+0x16` / `+0x1d` / `+0x1e`, aged by
  `0x100338e0`.
- Visuals: boom-type choice (section 1, step 4), explosion spawn `0x10048ff0` (animations on the voice
  generator), draw state `0x1000e4b0`, hull and dead sprites `0x1000eeb0`, red-circle call `0x10082630`.
