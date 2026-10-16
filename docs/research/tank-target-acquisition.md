# Tank target acquisition and attack orders

Research for issue #3 (map #1, Tank skirmish). Question: how a tank picks what to fire at on its own, how
hold position and cease fire change that, and what the tank orders ATTAQUER, ASSAUT, AVANCER, PIVOTER and
HALTE do to its target and mode.

Source: `Game_Dll.dll` from Sudden Strike Gold, read with Ghidra 12.1.4 through REA and with
`llvm-objdump`. The description values quoted come from the local `desc_common` files (`HU/*.COMMON`,
`HU/88`, `HU/SHOT2.HMG.X2`, `TARGETTYPES`). **Read** means the code at that address was checked;
**inferred** means a deduction from the code, not a direct reading. The manual (`manuel SS1.pdf`) is
quoted only where it agrees with the code.

Units used below: a unit's map position is `cell * 32 + offset / 256` on each axis (`0x100334c0`), so
ranges, dead zones and distances are in 1/32 of a cell. "Crew value" is the crew condition `+0x3d` raised
for veterans (`+0x1a & 0xc000`: at least `0xccd`, plus half, capped at `0x8000`), the same rule as in
`docs/original-code.md`. A pair `a,b` in a description is stored as `a` and `b - a` and read as
`a + (b - a) * crew >> 15`.

**Vtable.** Every slot below is the tank vtable **`0x100a8d24`**, the one the tank factory `0x10042fa0`
installs. `docs/original-code.md` calls `0x100a83f0` the tank vtable; another ticket found that it is
the apc's. Each slot used here was checked in both tables.

- Identical in both: `+0x1c`, `+0x20`, `+0x24`, `+0x28`, `+0x58`, `+0x80`-`+0x90`, `+0xb0`, `+0xc0`,
  `+0xc8`-`+0xd0`, `+0xec`-`+0xf4`, `+0x10c`, `+0x11c`-`+0x128`, `+0x144`, `+0x148`, `+0x160`,
  `+0x170`-`+0x17c`, `+0x1c4`, `+0x1c8`, `+0x1d0`, `+0x1d4`, `+0x1e4`, `+0x1e8`, `+0x2a8`-`+0x2bc`.
- Different for the tank:
  - `+0x10` capabilities = `0x10010510`;
  - `+0x14` right-click bits = `0x10011ec0`;
  - `+0x18` = `0x100101f0`;
  - `+0xc4` idle = `0x1000f6f0`. The apc's `0x10013800` adds an unload step first.
  - `+0x1a8` update = `0x10010b20`, with no crew step.

## Answer in brief

- A tank looks for targets inside its current order handler, through vtable `+0x2bc` (`0x10010c70`). It
  first drops any current target that is no longer valid, then runs a full search only when its scan
  timer `+0xc6` is 0. The timer counts down by one per tick (`0x1001702c`) and is reloaded after each
  search with `ScanDelay + rand(0..15)` ticks when a hostile is within twice the gun's range, otherwise
  `2 * ScanDelay + rand(0..31)`.
- A candidate must be alive, belong to a player hostile to the tank's player, and stand on a cell its
  side sees now (`0x10033440`). The search covers a square of cells around the tank sized from the
  main gun's range.
- Each weapon picks its own target. The main gun (weapon 0) scores a candidate by the type's
  `AttackPref` for the target's `TargetType` times how close the candidate is within range. The second
  weapon (weapon 1, the machine gun) scores by closeness times how far the candidate is from the hull
  axis. Both add 1/8 to keep the current target. Candidates beyond the real range, but within a bonus
  margin, can still be picked when nothing in range scores higher. Weapon 0 skips anything closer than its
  dead zone; weapon 1 divides those by 4. A weapon whose shot needs a line of fire also skips blocked
  candidates.
- Cease fire (`+0x1a` bit 5) skips the search in every automatic order: the turret turns back to the hull.
  An explicit ATTAQUER still fires. Hold position (`+0x1a` bit 4) only stops the automatic pursuit of an
  attacker (order 0xb). A tank that has been shot at chases its attacker if the attacker is within
  `AlarmRange` cells, then drives back to where it started.
- The orders, from grid command to queued record to unit order state:
  - AVANCER: command 0x13, record 0x41, order 2. This is the same move as a right-click on the ground.
    The tank fires on the move and never stops for it.
  - ATTAQUER on a unit: command 0x14, record 0x83, order 3. Both weapons are set to the target, and the
    tank chases it until it is in range.
  - ATTAQUER on the ground: command 0x14, record 0x42, order 4. The tank drives toward the point until
    it is in range, fires the gun, and the order ends.
  - ASSAUT: command 0x15, record 0x49, order 0xa. The tank moves; on sight of a target it stops,
    chases and fires (bonus margin 192 instead of 128), then moves on.
  - PIVOTER: command 0x1d, record 0x4f, order 0x13. The tank turns its hull to the point.
  - HALTE: command 0xf, vtable `+0xc0`. Clears the queue, the targets, the scan timer, the movement, the
    attacker and the order.
- Determinism: a search draws the shared generator once (twice when the tank has no rounds left). Every
  voice report also draws it, even though OpenStrike plays no sound (see the last section).

## The search, `0x10010c70` (tank vtable `+0x2bc`)

**Read.** Its three arguments are the range bonus for weapon 0, the range bonus for weapon 1, and a flag
that lets weapon 1 aim in any direction while the tank moves. It is called with `(0x80, 0x30, 1)` by idle
(`0x1000f75e`), the pursuit order 0xb (`0x10016705`), the attack-move order 0xc (`0x1000f922`) and the
other automatic orders, with `(0x80, 0x30, 0)` by the move order 2 (`0x1000fa21`), and with
`(0xc0, 0x30, 1)` by ASSAUT, order 0xa (`0x1000fab4`).

### 1. Keeping or dropping the current targets (every call)

The weapon targets are `+0xb8` (weapon 0) and `+0xbc` (weapon 1). For each one that is set, the tank asks
the target's vtable `+0x88` with the tank's player (`0x10010c99`, `0x10010cc6`). If the target is no
longer valid, that target is cleared and `+0xc6` is set to 0, so the full search runs at once in the same
call. When either target was set at entry, the "had no target" flag used for the report at the end is
cleared.

Validity is `0x10033440`, the tank's `+0x88`, with player `p`. The candidate must:

- be alive (vtable `+0x80`, `0x10033410`: `+0x1a` bit 0 clear);
- belong to a player whose bit is in `p`'s hostile mask `[0x101ff390 + 2 * p]`;
- stand on a cell whose seen bits `+0x22` contain `[0x101ff3c8 + p]`.

The seen bits are copied each tick from the cell's sight byte by `+0x188`. `[0x101ff3c8 + p]` is
`1 << side(p)`, written by `0x1004ef51` while the hostility masks are built.

**Correction for `docs/original-code.md`:** its map-array table lists `0x101ff3c8` as "per player: mask
of known mines", but `0x1004ef20` writes it as the player's side bit, and all the sight tests use it that
way.

So targets are **hostile only** and **seen now only**. Range is not part of validity: a target that moves
out of range is kept until the next full search replaces it.

### 2. When the full search runs

- The search runs only if `+0xc6` is 0 after step 1 (`0x10010cdd`).
- `+0xc6` is decremented once per tick, while it is non-zero, at the start of the tank update
  (`0x10017020` -> `0x1001702c`). That happens before the order dispatch `0x10032950` in the same update
  (`0x10010b20`).
- At vehicle creation `+0xc6` is set to a random value from 0 to 31 (`0x100171ef` -> `0x10001b80`, one
  draw), so tanks do not all search on the same tick.
- At the end of a search (`0x1001137c`), the tank compares the distance to the nearest valid hostile it
  saw with twice weapon 0's search range.
  - If that hostile is farther (or there was none), it sets `+0x1a` bit 7 and loads
    `+0xc6 = max(+0xc6, 2 * ScanDelay + (rng >> 16 & 31))` (`0x10011388`).
  - Otherwise it clears bit 7 and loads `+0xc6 = max(+0xc6, ScanDelay + (rng >> 16 & 15))`
    (`0x100113d2`).
  - In both cases one draw of the shared generator.
- `ScanDelay` is the type field `+0x270` (key `scandelay`, parser `0x10046ffd`; default -1,
  `0x1004639c`). In the tank commons it is 20 for light tanks, 40 for medium tanks and assault guns, and
  80 for heavy tanks.
- *Inferred:* bit 7 of `+0x1a` means "no enemy near"; no reader was looked for.
- *Unconfirmed:* with the default -1 the jittered value can be negative and would be stored as a huge
  16-bit delay. Whether any of the 47 tanks lacks `ScanDelay` was not checked.

The timer is also zeroed, which forces a search on the next call, in three cases:

- when a unit is destroyed, through the broadcast in `0x1003edc0`: every unit's `+0x1e8`, which for a
  tank is `0x10017410`, clears the weapon targets that point at the dead unit;
- when a unit changes player, through `0x1003ecc0` -> `+0x1e4` (`0x10017450`);
- by HALTE and by the idle reset (`0x100165a0`).

### 3. Candidates

- The rectangle of cells is centred on the tank's cell (`0x1004c4d0` -> `0x1004c500`). Its half side is
  `((R0 + 0x70) >> 4) >> 1` cells, where `R0` is weapon 0's search range below.
- The rectangle is clipped to the map. The candidates are linked through `+0x12`, cell by cell: rows
  from the top, cells from the left, each cell's chain from `0x101854e6` / `+0x0e`. Each unit is
  **prepended**, so the list runs from the bottom-right cell backwards.
- Ties keep the first candidate met (strict comparisons), so this order matters.
- *Inferred:* the box uses weapon 0's range even when weapon 1 reaches farther or weapon 0 has no
  rounds.
- The tank itself is in the box and is rejected by the hostility test.

For each candidate (`0x10010e9f` loop):

1. Validity `+0x88` as in step 1, else skip.
2. Its position and distance from the tank's pixel `[0x100c4514/8]`, through the candidate's `+0x90`
   (`0x100334c0`). The distance is `max(|dx|, |dy|) + min(|dx|, |dy|) / 2`. The smallest distance seen
   is kept for the timer of step 2.
3. Weapon 0 and weapon 1 are evaluated in turn, each with its own best score (start -1,000,000) and best
   candidate.

Ranges, all crew-interpolated (type offsets; weapon `i` record at `+0xb70 + 0x93 * i`; keys from
`0x10048250`):

| Value | Weapon 0 | Weapon 1 | Key |
|---|---|---|---|
| range | `+0xbbb`, `+0xbbf` | `+0xc4e`, `+0xc52` | `shotN_Range` |
| dead zone | `+0xbc3`, `+0xbc7` | `+0xc56`, `+0xc5a` | `shotN_DeadZone` |
| ammunition kind, rounds per shot | `+0xbdb`, `+0xbdf` | `+0xc6e`, `+0xc72` | `shotN_UseAmmo` |

The search range `R0` is weapon 0's range plus the first argument; `R1` is weapon 1's range plus the
second. A weapon "has rounds" when the tank's ammunition `+0x39 + 2 * kind` is at least its rounds per
shot (`0x10010e04`-`0x10010e33`).

**Weapon 0 (the gun)**, if it has rounds and the distance is at most `R0` (`0x10010ef2`):

1. The candidate is skipped if it is closer than the dead zone (`0x10010f93`).
2. The score is `(AttackPref[cat] + 256) * (((R0 - d) << 16) / R0) >> 9` (`0x10010f9b`-`0x10010fd4`):
   - `cat` is the candidate type's `TargetType` index `+0xcc` (`0x10046f3a`);
   - `AttackPref` is the tank type's table `+0xd0 + 4 * cat` (key `attackpref`, `0x10046fa6`; 8 entries,
     default 0).
3. The score gains `score >> 3` if the candidate is the current `+0xb8` (`0x10010fdb`).
4. The score loses 100,000 if the distance is beyond the real range, without the bonus (`0x10011037`).
5. If the score beats the best, the line of fire is checked when the shot needs one. The shot record is
   the pointer at weapon `+0`; its field `+0x20` is non-zero for such shots.
   - The check is `0x1004dc90` from the tank's pixel to the candidate's. It steps
     `max(|dx|, |dy|) / 32` times and tests the cell flags bit `0x800` (`0x101054e6`) at each point in
     between, not at the two ends.
   - If the line is clear, the candidate becomes weapon 0's best, and the line is marked as already
     checked for weapon 1.
   - If it is blocked, the candidate is skipped entirely when weapon 1 has no rounds or weapon 1's shot
     also needs a line of fire. Otherwise it goes on to weapon 1 (`0x10011074`-`0x10011090`).

The category indices follow `desc_common/TARGETTYPES`: ATG, bigeyes, dom, fast, longdistance, man, soft,
tank. *Inferred:* the parser `0x1004a800` searches a 16-byte name table at `0x100c50cc`; the order of that
table was not traced to its loader. The tank commons include `EXPLO.ATTACK.PREFS`: Tank 256, Soft 128,
Fast 64, Man 32, Dom 64.

**Weapon 1 (the machine gun)**, if it has rounds and the distance is at most `R1` (`0x100110a9`):

1. The candidate's vtable `+0xb0` (tank `0x10033550`) must return at least 2 for weapon 1's record and
   the tank's crew value (`0x10011128`).
   - *Inferred:* this is an expected-damage estimate: the target type's table entry for the shot's
     damage kind times the shot's `Damage` pair, `>> 8`. In effect the gun ignores targets it cannot
     hurt.
2. The angle from the tank to the candidate (32 headings, the same formula as `0x1002e2c4`) is compared
   with the **hull** heading `+0x63`. The difference, from 0 to 16, must not exceed the angle limit
   (`0x100111b2`).
   - The limit is 17 (any direction) when the tank is not immobilised (`+0xb3`) and either is not
     moving (`+0x1a` bit 1) or the third argument is set.
   - Otherwise the limit is 1 (`0x10010cea`-`0x10010d0a`).
   - So a moving tank in the move order, or an immobilised one, uses its machine gun only within one
     heading of the hull front. *Inferred:* the hull gun.
3. The score is `(((R1 - d) << 16) / R1) * (18 - angle) >> 4`, with no `AttackPref` (`0x100111bc`-
   `0x100111e4`).
4. The score gains 1/8 if the candidate is the current `+0xbc`.
5. The score loses 100,000 beyond the real range, or is divided by 4 inside the dead zone
   (`0x10011271`-`0x100112d4`).
6. If the score beats the best, the line of fire is checked (if not already done for this candidate)
   when **weapon 0's** shot needs one (`0x100112dd` reads the type's `+0xb70`).
   - *Inferred quirk:* weapon 1's own flag is not consulted here. Keep this when reimplementing.

### 4. After the loop

- `+0xb8` and `+0xbc` are set to the two winners (`0x10011346`); either may be 0.
- The tank then makes a voice report through vtable `+0x1c8` (`0x10033eb0`) with kind 0
  (`0x10011370`) when either of these holds:
  - it had no target at entry and now has a weapon 1 target;
  - it has a weapon 0 target and both reload counters `+0xc0`, `+0xc3` are 0.
- If neither weapon had rounds, one draw before the loop (`0x10010e44`): one time in 8
  (`rng & 0x70000 == 0`) the tank makes report kind 4.
- Then the timer of step 2.

## Order states and what each does with targets

The order dispatch `0x10032950` calls vtable `+0xc4 + 4 * (order - 1)` (order 0x16 uses `+0xd8`). When a
handler returns non-zero, the next queued record is taken (`0x100326b0`), or the tank is reset to idle
(`+0x11c`) if the queue is empty.

Queued records are 7 bytes: next u16, code, then a 4-byte argument (`0x10032060`, `0x10032250`). The
record dispatcher `0x10032740` maps a code through the byte table `0x100328c4` (index `code - 0xd`) and the
jump table `0x10032870` to a vtable entry. It clears `+0x25`, `+0x27` and `+0x29` first.

| Record | Tank entry | Sets |
|---|---|---|
| 0x41 move | `+0x120` `0x1002d290` | order 2 (known move) |
| 0x42 attack point | `+0x128` `0x10016620` | order 4, `+0x25` = point (x, y in 1/32 cell) |
| 0x49 assault | `+0x144` `0x10017670` | order 0xa, `+0x25` = cell |
| 0x4a attack-move | `+0x148` `0x100165c0` | order 0xc, `+0x25` = cell |
| 0x4f pivot | `+0x160` `0x10023d80` | order 0x13, `+0x25` = point (1/32 cell) |
| 0x83 attack unit | `+0x124` `0x100165e0` | order 3, `+0x29` = `+0xb8` = `+0xbc` = target (refused if null or itself) |

Order handlers (tank vtable `0x100a8d24`):

- **1 idle, `0x1000f6f0`** (read). With cease fire, the turret target heading `+0xcf`
  becomes -1, so the turret returns to the hull, and nothing else happens.
  - Otherwise: search `(0x80, 0x30, 1)`, then `0x100169b0` aims and fires both weapons at their targets
    through `+0x2ac` and `+0x2b0` (firing, issue #4).
  - It sets `+0xcf` to weapon 0's aim. When the tank is not moving, it sets the wanted hull heading
    `+0x64`: toward weapon 1's aim if weapon 1 can engage, otherwise toward weapon 0's (`0x1000f7b1`).
  - The idle tank does **not** move toward targets out of range. It only turns.
  - Then the attacker reaction below.
- **2 move, `0x1000f9f0`** (read). The order ends when the drive arrives or fails (`0x1002d320`). With
  cease fire it sets `+0xcf` = -1 and fires nothing.
  - Otherwise: search `(0x80, 0x30, 0)`. It aims weapon 0 at `+0xb8` (`+0x2ac(0)`) and fires when
    ready; it aims weapon 1 at `+0xbc` (`+0x2ac(1)`) and fires when ready. The movement is never
    stopped. This is the manual's "tire sur ses ennemis éventuels sans s'arrêter".
- **3 attack unit, `0x10016820`** (read). While `+0xb8` is set, it runs the single-target attack
  `+0x2b4`. It ends when that returns 2, or when `+0xb8` has been cleared by the death broadcast. No
  search runs and cease fire is not tested, so the ordered target is kept even out of sight.
- **4 attack point, `0x1000fb10`** (read). `+0x2a8` (`0x1000fd10` -> `0x100011b0`) aims weapon 0 at the
  point and sets `+0xcf`.
  - With no rounds: stop, end.
  - Out of range (flag `4` of `[0x100c4540]`): if it is not already driving to a cell, it starts
    driving to the point's cell (`0x1002d100(1, point >> 5)`); if the drive failed (status 3), stop and
    end.
  - In range: stop. Inside the dead zone, end. Otherwise fire when ready (`+0x2b0`), and end once the
    weapon's counter `+0xc2 + 3 * weapon` is 0. *Inferred:* the burst is done, so one shot or one burst
    per order.
  - Only weapon 0 is used.
- **0xa ASSAUT, `0x1000faa0`** (read). Without cease fire: search `(0xc0, 0x30, 1)`. If there is a target,
  it runs the engage `+0x2b8` (`0x10010010`) and stays on it until that returns 2. With no target (or
  under cease fire) it sets `+0xcf` = -1 and drives to `+0x25`, using the move handler once a cell goal
  is set. It ends on arrival.
- **0xb pursuit, `0x100166e0`** (read). Set only by the attacker reaction.
  - It ends when the tank has hold position or cease fire, when it has no attacker `+0x29`, when the
    engage returns 2, or when the tank is more than `AlarmRange` cells from its anchor `+0x25`.
    - The anchor is the cell where the pursuit began.
    - Distance in cells is `max + min / 2`.
    - `AlarmRange` is type `+0x2a0`, `+0x2a4`, crew-interpolated (key `alarmrange`, `0x100472a1`).
      Heavy tanks 3,4; medium 7,8; light 11,12.
    - *Unconfirmed here:* another ticket reports that `shot1_Range >> 5` is added to `AlarmRange` after
      parsing. The parser `0x1004729e` stores the pair as it is.
  - Otherwise: search `(0x80, 0x30, 1)`. If neither weapon found a target, both are set to the attacker.
    Then the engage `+0x2b8` runs.
  - On ending it stops, sets order 0xc and clears `+0x29`, so the tank attack-moves back to the anchor.
  - While it continues, it returns "done" only when a queued order is waiting.
- **0xc attack-move, `0x1000f900`** (read). Without cease fire: search `(0x80, 0x30, 1)`, then
  `0x100169b0`.
  - If either weapon has something to shoot, the tank stops its drive and turns to it.
  - If neither does, it drives to `+0x25` and ends on arrival.
- **0x13 PIVOTER, `0x10016640`** (read). It first runs the idle handler (`+0xc4`), so it still searches
  and fires. Then it sets `+0x64` to the heading from the tank's pixel to the point. It ends when the hull
  heading equals it, or at once when the tank is immobilised.

The single-target attack `+0x2b4` (`0x1000fe40`) and the engage `+0x2b8` (`0x10010010`) are read.

- The engage copies one target to the other weapon when only one is set and calls `+0x2b4`.
- Otherwise both use `0x10016b30`, which aims each weapon (`+0x2ac`) and fires when ready (`+0x2b0`),
  then reports which weapon is out of range.
- When every weapon reports its target out of range, unseen or blocked, the tank checks it can move
  (`0x10016890`: not immobilised, and a first step is possible, `0x1002cf40`). It then follows the target
  as a unit goal (`0x1002d180(2, target)`: movement goal kind 2), with one draw for a report of kind 1
  (one time in 4, `rng & 0x30000 == 0`), and returns 1.
- When it is in range, it stops (`0x1002d200`), turns the hull to the target when not moving, and keeps
  firing.
- It returns 2 (give up, the order ends) when the tank cannot move, or when the follow drive failed
  (`[0x100c4510]` = 3).
- *Inferred:* an ordered target out of sight is followed by its true position, because the follow goal
  reads the unit itself.

The aiming flags `[0x100c4540]` come from `0x10001540` (read); they are for the firing ticket.

- bit 0: the target is not seen by the tank's side;
- bit 1: inside the dead zone;
- bit 2: unseen, beyond the real range, or (for line-of-fire shots) blocked;
- bit 3: the turret or hull is not aligned yet (`+0x2ac`);
- bit 4: no rounds;
- bit 5: ready to fire (none of bits 1, 2 and 4, then aligned and reloaded).

## The attacker reaction (automatic pursuit)

- A hit on a unit goes through its vtable `+0x1d0` (`0x10032c40`, read; called from `0x10041e98` and
  `0x10041f45`, damage, issue #5).
  - It applies the damage through `+0x1cc` and makes voice report 2 (once more than 200 ticks have
    passed since the last hit, `+0x42`).
  - If the shooter is hostile and seen by the victim's side, it records the shooter in `+0x16`. The age
    `+0x1d` is reset to 0 and `+0x1e` holds the result of `+0x1cc`.
  - It then calls `0x1003ef80`, which passes the shooter to the vtable `+0x178` (`0x10032640`) of every
    live unit within 2 cells of the victim (a 5 x 5 box). `0x10032640` records it the same way, with
    `+0x1e` = 0, under the same hostile-and-seen test.
  - So nearby units learn of the attacker too. The other caller of `0x1003ef80` is `0x1002f9b1`.
- `+0x1d` is incremented each tick (`0x100338e0`, from `0x1002dd80` in the tank update). The attacker is
  forgotten after 200 ticks.
- In idle, a tank without hold position and with an attacker younger than 64 ticks pursues it if the
  attacker is closer than `AlarmRange` cells (`0x1000f7c4`-`0x1000f893`). The idle handler then sets
  order 0xb, `+0x29` = the attacker and `+0x25` = the tank's own cell.
- Hold position (`+0x1a` bit 4, set by `+0x174`, `0x10032c00`) prevents this and ends a running pursuit.
  This matches the manual: "les unités ne se déplacent pas automatiquement pour attaquer l'ennemi". No
  other tank code was found to test it except weapon 1's handling in `0x10016b30`.
- Cease fire (`+0x1a` bit 5, set by `+0x170`, `0x10032bc0`) is tested by idle, move, ASSAUT, pursuit and
  attack-move. It is not tested by attack unit (order 3) or attack point (order 4).
- Setting either flag changes nothing else. The current targets stay, but are not used while cease fire
  is on.

## From the orders grid to the unit

A target click with an armed command is handled by `0x100809f0` (switch on the armed command). The
command is queued with `0x1005e070` (a point) or `0x1005e140` (a point and the unit under the cursor,
`0x10070810`). It is executed later by `0x1005eb30`, on code, for the units of that player that are
selected (`+0x1a & 8`).

| Grid command | Click check | Queued code | Executor | Per-unit check | Group order |
|---|---|---|---|---|---|
| no armed order (right click) | - | 0xc4 | `0x10061490` | `+0x14` (tank `0x10011ec0`) gives bits, the lowest set bit across the selection wins; `+0x18` (`0x100101f0`) then confirms each unit | bit 4 (hostile seen unit) -> attack unit `0x10060cb0`; bit 0x200 (ground at least 4 cells from the edge) -> move `0x10060bd0(.., 1)` |
| 0x13 AVANCER | `0x10083a10` | 0x49 | `0x1005f8e0` | `+0x1c` (`0x1001b860`, always 2) | `0x10060bd0(cell, 0)` -> `0x100609c0` record 0x41 with formation |
| 0x14 ATTAQUER | `0x10083c50` | 0xca | `0x1005fbe0` | unit under the cursor valid for `+0x8c` (`0x10033490`: alive and seen, **no hostility test**): `+0x24` (`0x10010550`); otherwise the ground: `+0x28` (`0x100108b0`) | unit: `0x10060cb0` record 0x83; ground: `0x10060d10` -> `0x10060920` record 0x42 |
| 0x15 ASSAUT | `0x10083aa0` | 0x4b | `0x1005fb60` | `+0x20` (`0x10016f30`: rounds for a weapon) | `0x10060c70` -> `0x100609c0` record 0x49 with formation |
| 0x1d PIVOTER | `0x10084190` | 0x53 | `0x10060060` | `+0x58` (`0x10016f80`: 2 if mobile) | `0x10060c90` -> `0x10060920` record 0x4f, no formation |
| HALTE | - | 0xf | `0x1005f880` | - | every selected unit's `+0xc0` |
| hold fire, hold position | - | 0x54, 0x55 | `0x100605a0`, `0x10060560` | - | `+0x170`, `+0x174` with the value |

- Each group order first runs the unit's HALTE (`+0xc0`), unless Shift is held (bit 0 of the command's
  modifiers, `in_ECX + 8` in `0x10060cb0`, `0x10060920`, `0x100609c0`, `0x10060ac0`). The record is
  therefore queued after the current orders when Shift is held. A queue across orders is out of scope on
  the map; only Shift keeping the command armed is in.
- Visual hooks (issue #6):
  - attack unit sets `+0x1b |= 0x3c` on the **target** when the shooter is the local player's
    (`0x10060cb0`);
  - attack point draws one marker at the point for the local player (`0x100409d0(x, y, 0, 1, 0,
    [0x10382944], 1)`, inside `0x10060920`);
  - `+0x1b |= 0x3c` is also the mark the group orders put on the unit itself (`0x10060920`,
    `0x100609c0`).
  - *Inferred:* both are the attack reticle or a blinking marker.

**AVANCER is the right-click move for a tank.** Both send record 0x41. `0x10060ac0`, used by the right
click, sends 0x4a (attack-move) instead only for types with flag `+0x20 & 0x200000`.

- *Inferred:* that flag marks infantry, since the crew code tests it on crew member types at
  `0x10014c3f`.
- This matches the manual: for infantry DEPLACER does not fire and DEPLACER2, the default, does; for
  tanks DEPLACER fires on the move.
- The difference for a tank is only that AVANCER moves even when the click is on an enemy.

**ATTAQUER acceptance.**

- On a unit, `0x10010550` returns:
  - 2 when weapon 0 has rounds and the target is between its dead zone and range, or likewise for
    weapon 1 (weapon 1 also needs the hull within one heading when immobilised);
  - 1 when out of range and mobile, or when inside both dead zones and able to move;
  - 0 otherwise.
- On the ground, `0x100108b0` needs `CanAttackPoint` (type `+0x29c`, key `canattackpoint`,
  `0x1004726f`; 1 in every tank common) and weapon 0's rounds. It returns 2 when beyond the dead zone
  and in range, 1 when out of range and mobile, and 0 when inside the dead zone.
- *Inferred:* because `+0x8c` has no hostility test, an attack can be ordered on a seen friendly unit.

**HALTE** is tank `+0xc0` = `0x10032690`. It frees the order queue `+0x2d` (`0x10031d70`), then calls
`+0x11c` (`0x1000f6d0`):

- the turret delay `+0xd0` is raised to at least 2;
- `0x100165a0` clears `+0xb8`, `+0xbc` and the scan timer `+0xc6`, so a search follows;
- `0x1002d240` resets the movement (goal kind `+0xa8`, path state, counters);
- `0x1002ccd0` sets the wanted heading `+0x64` to the hull heading;
- `0x10032b10` clears `+0x29`, the attacker `+0x16`, `+0x1d` and `+0x1e`, sets order `+0x24` = 1 (idle)
  and sets `+0x25` = the tank's cell.

It does not touch the hold flags, the turret heading or the reload counters.

## Fields involved

| Field | Meaning |
|---|---|
| `+0x16` | last attacker (unit pointer), `+0x1d` its age in ticks (forgotten above 200, pursued below 64), `+0x1e` cleared with it |
| `+0x1a` bit 1 | moving; bit 4 hold position; bit 5 cease fire; bit 7 no enemy within twice the gun range (inferred) |
| `+0x22` | the cell's seen bits this tick, per side |
| `+0x24` | order state: 1 idle, 2 move, 3 attack unit, 4 attack point, 0xa assault, 0xb pursuit, 0xc attack-move, 0x13 pivot |
| `+0x25` | order point or cell (x, y i16), pursuit anchor |
| `+0x29` | order target unit (attack, pursuit) |
| `+0x2d` | order queue head (u16, -1 none) |
| `+0x64` | wanted hull heading; `+0x63` hull heading |
| `+0xa8`, `+0xa9` | movement goal kind (1 cell, 2 unit) and goal |
| `+0xb3` | immobilised |
| `+0xb8`, `+0xbc` | targets of weapon 0 and weapon 1 |
| `+0xc0`, `+0xc3` | reload counters of weapon 0 and 1 (counted down in `0x10017020`), `+0xc2 + 3 * w` burst counter (inferred) |
| `+0xc6` | ticks until the next full search (u16) |
| `+0xcf` | turret target heading (-1 = follow the hull) |
| type `+0xcc` | `TargetType` index |
| type `+0xd0 + 4 * cat` | `AttackPref` |
| type `+0x270` | `ScanDelay` |
| type `+0x29c` | `CanAttackPoint` |
| type `+0x2a0`, `+0x2a4` | `AlarmRange` (cells) |
| type `+0xb70 + 0x93 * w` | weapon records: shot pointer (`+0x20` of the shot: needs line of fire, inferred), range `+0x4b`, dead zone `+0x53`, ammunition kind `+0x6b`, rounds per shot `+0x6f` |

## Shared random generator `0x10299130`

All of these run inside the unit update `+0x1a8` of the tick loop `0x1003e520`, in unit list order, in
the order handler called after the timers.

| Address | When | Draws |
|---|---|---|
| `0x100171ef` -> `0x10001b80` | vehicle creation | 1: initial `+0xc6` = 0-31 |
| `0x10010e44` | each full search of a tank with no rounds for either weapon | 1 (1 in 8: report kind 4) |
| `0x1001138c` or `0x100113d8` | end of each full search | 1: timer jitter |
| `0x1000fec4`, `0x1000ff0c`, `0x1000ffa1`, `0x100100ed`, `0x1001017e` | the attack starts following a target | 1 (1 in 4: report kind 1) |
| `0x10033bcc` or `0x1003f073` | **every voice report** (`+0x1c8` -> `+0x1c4` `0x10033b80`) | 1 |

The last row matters even without sound.

- The report entry `0x10033b80` checks the per-kind cooldown `+0x46 + 4 * kind`. While the cooldown is
  running it draws once and shortens it by 0-3 ticks.
- Otherwise it plays the sound if seen locally, then `0x1003f070` draws once to set the next cooldown,
  and walks the units within 12 cells (inferred: to share it).
- The reports from this ticket are kind 0 (new target, `0x10011370`), kind 1 (start chasing) and kind 4
  (no rounds).
- So the reports, their cooldowns and the neighbour walk must be simulated for the generator to stay
  aligned. The damage path also makes one: kind 3, one time in 2, in `0x10032e7f`.

## Open questions and notes for the other tickets

- The neighbour walk of `0x1003f070` and the cooldown fields `+0x46` were not read in full. They are
  needed for the generator sequence.
- *Unconfirmed:* every tank type sets `ScanDelay`; with the default -1 the delay would wrap.
- *Inferred:* the line-of-fire test for weapon 1 uses weapon 0's shot flag (`0x100112dd`), and the
  candidate box uses weapon 0's range.
- The meaning of `+0x1a` bit 7 (no enemy near) and of `+0x1b |= 0x3c` (marker) were not checked against
  their readers.
- For damage (issue #5):
  - `0x10032de0` changes hit points: it heals up to the maximum, or at 0 calls the death broadcast
    `0x1003edc0` with a state stored in `+0x41`;
  - `0x1003ef80` is the "attacked" notification;
  - `0x10033550` is the expected-damage estimate (another ticket reports that it uses the front armour
    only);
  - idle tanks call `0x1003ef50` -> `+0x1d4` (`0x1002dbe0`) every 64 ticks on their phase when their hit
    points are between 1/4 and 3/4 of the maximum. *Inferred:* self-repair by 1, which also updates
    `+0xb3`.
- For firing (issue #4): `0x10001540` / `0x100011b0` (aim at a unit or a point, flags
  `[0x100c4540]`, heading `[0x100c453c]`), `+0x2ac` `0x1000fbe0`, `+0x2b0` `0x1000fdf0`, `0x10016b30`,
  `0x100169b0`.
