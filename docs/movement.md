# Vehicle movement (from `Game_Dll.dll`)

How the original moves ground vehicles, as read in `Game_Dll.dll` (Sudden Strike Gold). OpenStrike's
`crates/sim` follows it; the remaining gaps are listed at the end. Offsets such as `unit + 0x63` are
fields of the unit object (tank vtable `0x100a83f0`); `type + 0xb33` are fields of its unit type.

## Time

- One tick per `[0x10388460]` ms (scheduler `0x1005b920`). Single player sets 40 ms (`0x10090ffa`),
  so **25 ticks per second**; the in-game speed setting gives `60 - 4 * level` ms (`0x1007eaab`).
- Per tick (`0x1003e520`): every unit's `+0x184` (one unit in 64 per tick), `+0x188`, then each
  unit's update `+0x1a8` in list order, then the path scheduler (`0x100405c0`).
- One random generator for everything (`0x10299130`): `x = x * 0x41c64e6d + 0x3039`, the result
  used as `x >> 16`.

## Positions and the map grid

- A unit is in a 32 px **cell** (`+0x2f`, `+0x31`) at an offset (`+0x33`, `+0x35`) in 1/256 px from
  the cell's top-left corner, so `0x2000` per cell. The offset leaves the cell while driving out.
- The movement grid is one 32-bit word per cell in a 256 x 256 array (`0x101054e6`): bits 0-3 block
  the 16 px quarters (`qx + 2 * qy`), bit 14 keeps vehicles out, bits 20-23 are mines known per
  player, bit 30 marks the border. The map loader (`0x1004c980`) copies the map's words, sets bits
  19-29 from later map sections, and forces the 4 outermost cells of each side to `0x4000000f`.
- Neighbour steps (`0x100b2144`): 0 east (1, 0), then clockwise with y down: 1 (1, 1), 2 (0, 1),
  3 (-1, 1), 4 (-1, 0), 5 (-1, -1), 6 (0, -1), 7 (1, -1); 8 is "no step". Direction from signs:
  table `0x100b2130` indexed by `sx * 3 + sy`.

## Entering a cell (`0x1002cf40`)

A vehicle (type `Engine` not `none`, `type + 0xb2f`) may step from `c` into neighbour `n` when:

- no unit holds `n`: `occupancy[n] & 0xffff == 0`;
- all 4 quarters of `n` are open and bit 14 is clear: a vehicle needs a **whole free cell**;
- no mine known to its player is there, unless the type has `IgnoreMines`;
- for a diagonal step `d`, the side cells' quarters next to the diagonal are open:
  `grid[(nx, cy)] & {4, 8, 2, 1}[d >> 1]` and `grid[(cx, ny)] & {2, 1, 4, 8}[d >> 1]`
  (`0x100b218c`, `0x100b2190`), and their occupancy is clear under masks
  `{0xf00, 0xf000, 0xf0, 0xf}` and `{0xf0, 0xf, 0xf00, 0xf000}` (`0x100b222c`, `0x100b2230`).

## Occupancy (`0x101454e6`)

One word per cell: 4-bit counters per quarter in the low half and again in the high half. A unit
keeps a reservation (`+0x66`: cell, direction it entered by, layer). Moving it (`0x1002c740`) removes
the old one (`0x1002c970`) and adds the new one (`0x1002c7a0`): `0x11111111` on its cell; while
entering by direction `d`, also `0x11111111` on the cell it comes from and, for a diagonal,
`{0x100, 0x1000, 0x10, 0x1}[d >> 1]` on `(x, y - dy)` and `{0x10, 0x1, 0x100, 0x1000}[d >> 1]` on
`(x - dx, y)` (`0x100b027c`, `0x100b0280`). Layer 1 (an immobilised vehicle, `+0xb3`) uses other
tables (`0x100b023c`, `0x100b025c`), not reproduced yet.

## Speed and turning

- Unit type keys (`0x100479c0`): `MoveSpeed a,b` stores `trunc(a * 256)` and
  `trunc(b * 256) - trunc(a * 256)` (`type + 0xb33`, `+0xb37`); `TurnDelay a,b` stores `a` and
  `b - a` (`+0xb27`, `+0xb2b`); `GunTurnDelay` (`+0xc9e`), `IgnoreMines` (`+0xb43`), `MoveDamageHP`
  (`+0xb47`), `CanMoveBackward` (`+0xb64`), `Expa_Crush a,b` (`+0xb3b`, `+0xb3f`).
- Crew condition `k` (`+0x3d`, 0 to `0x8000` for an unhurt crew). Speed (`0x100142e0`):
  `min + (range * k >> 15)` in 1/256 px per tick, scaled by the towing factor when towing.
  An unhurt T-26 (`MoveSpeed 1.4,1.5`) drives 384/256 = 1.5 px per tick.
- Hull turn (`0x1002ce60`): the heading (`+0x63`, 32 steps clockwise from east) moves one step
  towards the wanted one (`+0x64`, the shorter way, +1 at exactly 16) when the delay (`+0x65`) is 0,
  which then becomes `TurnDelay min + (range * k >> 15)` (3 for a T-26: one step every 4 ticks). The
  turret follows the hull steps while it has no target (`0x10010a60`) and turns on its own every
  `GunTurnDelay` ticks towards its target or the hull heading (`0x10010b20`).
- Headings: `trunc((atan2(dy, dx) + 2pi) / 2pi * 32 + 0.5) & 31` with the game's single precision pi
  (`0x100a8284`, `0x40490fda`) and its own arctangent (`0x1005b3f0`: 20-term series around 0 or 1,
  table `0x100b1de8`, whose last term has the wrong sign). Square roots use `0x1005b3a0` (start at
  the power of two above, 5 Newton steps; `sqrt(0)` is 1/32).

## Driving (`0x1002dee0`, each tick)

The unit drives towards a point (`+0xad`, `+0xaf`) of the current curve.

1. If the point is farther than `0xfa` (distance `max + min / 2`), the wanted heading is the
   heading to it (minus 16 when driving backwards; equal to the heading clears the backwards flag),
   the hull turns, and only if it now faces the wanted heading does it move by
   `min(speed, distance)` along the vector (integer division). Crossing into another cell's area
   draws a random number. Otherwise it turns in place.
2. Else it turns, snaps to the point and takes the next one. At the end of the curve
   (`0x100b7360`, `common/CURVENEW`, 81 curves indexed `next * 9 + previous`):
   - with a next path direction and a free cell (rule above): step the cell, move the reservation,
     shift the offset by `-0x2000` per axis, start the curve `(dir, previous)`. Types with
     `CanMoveBackward` then draw a number and drive backwards when the end of the curve is more
     than `14 + rnd & 1` headings away (first 5 steps of a path) or `8 + rnd & 7` (later).
   - otherwise, if not stopped yet: reserve only this cell and take the stopping curve
     `(8, previous)` to the middle of the cell, (15, 15) px.
   - stopped with nothing to do: arrived if in the goal cell, else ask for a path (status 4), or
     give the order up if the path finder failed (status 3).
3. The move order (`0x1000f9f0`) ends on status 2 (arrived) or 3 (failed) (`0x1002d320`).

## Path finder (vehicles: `0x10061e60`)

Paths are rings of 64 directions (`+0x7f` index, `+0x80` end, nibbles from `+0x82`), planned for
the units that want one by the scheduler `0x100405c0` after all units moved:

- Units are sorted into 256 buckets by `speed >> 3` (0 without an order or while waiting)
  (`0x1002da60`); the planning bucket adds `blocked_ticks >> 4` (`0x1002dab0`). Bucket by bucket,
  slowest first, the units of the bucket are planned, then the units of the same marking bucket
  are marked as obstacles (`0x1002cb40`) for the following ones. Units waiting (`+0xa3`) are skipped.
- The planner (`0x10067080`) searches once ignoring the marked units (first plan of an order), then
  once avoiding them, and keeps the second path if `(first length + 32) / (length + 32) >= 0.5` and
  its progress per cell is at least half the first one's; otherwise the unit waits 0-3 ticks
  (25-32 after 20 failures, `+0xa2`), 150 failures give the order up. A unit whose next step is
  blocked only by another unit just waits.
- The search (`0x10061f90`): label the straight line to the goal; run a ring of 32 walkers; a walker
  heads for the goal; blocked, it splits into two wall followers turning opposite ways, which head for
  the goal again once on a straight-line cell further along. Every reached cell keeps its walking
  distance; walkers at distance `0x3ff` stop. The closest cell to the goal (distance squared plus a
  quarter of the walk) is the end. Then walk back from it: look along 8 rays for the first reached
  cell, step towards the closest, until the start. At most 61 directions are kept.

## Not reproduced yet

- How a group order spreads the units (OpenStrike puts them on rings 2 cells apart around the
  target cell: own rule).
- Which sprite frame the game draws for a heading (OpenStrike projects the map heading).
- Crushing objects and mines while driving (`0x1004c710`), engine smoke and sounds, damaged crews,
  immobilised vehicles (layer 1), towing, infantry and other path finder variants (`0x100639d0`,
  `0x100656c0`), orders other than move.
