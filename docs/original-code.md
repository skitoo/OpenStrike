# What is known of the original code

An index of everything read so far in the shipped binaries, so that nothing has to be reverse
engineered twice. Details live in `docs/formats.md` (file formats) and `docs/movement.md`
(movement); this file maps addresses to meanings. Keep it up to date with every finding.

Binaries (`data/SuddenStrikeGold/`): `Game_Dll.dll` (game logic, image base `0x10000000`, `.text`
file offset = address - `0x10000000`), `cad800.dll` (drawing), `QED.EXE` (map editor).
Status: **read** = disassembly or decompilation checked; **seen** = known from a call site or data
only.

## Working method

- Ghidra 12.1.4 through the REA MCP server (`open_binary`, `procedure_pseudo_code`,
  `batch_decompile`, `resolve_containing_procedure`). Ghidra misnames some functions that have no
  entry (vtable targets): disassemble those instead.
- Full disassembly for grepping: `xcrun llvm-objdump -d --x86-asm-syntax=intel
  data/SuddenStrikeGold/Game_Dll.dll > game.asm` (same for `cad800.dll`). Search call sites with
  `grep "call	0x1002cf40"`, virtual calls with `grep "+ 0x2a4\]$"`, field users with
  `grep "+ 0xb64\]"`.
- Read constants with Python: `struct.unpack_from('<d', dll, addr - 0x10000000)`.
- Ghidra's pseudocode loses register aliases in big functions (it showed `param_2` for a computed
  x in the path search): check the assembly before trusting a surprising line.
- The C runtime `__ftol` (`0x1009c5f8`) truncates towards zero; x87 code runs in double precision.

## Main loop and time

| Address | Status | What |
|---|---|---|
| `0x1005b920` | read | tick scheduler: next tick += `[0x10388460]` ms |
| `0x10090ffa` | read | single player: 40 ms per tick (25 per second) |
| `0x1007eaab` | read | game speed setting: `60 - 4 * level` ms |
| `0x1003e520` | read | per tick: units `+0x184` (when `tick & 63 == unit+0x23`), all `+0x188`, all `+0x1a8` (update, sets `[0x100b7500]` = current unit, `[0x100c4514/8]` = its pixel), then `0x100405c0`, `0x1003cd00` |
| `[0x10388434]` | read | tick counter |
| `[0x100c4508]` | read | unit list head, next at `unit + 0x0a` |
| `[0x10299130]` | read | the game's random generator, `x * 0x41c64e6d + 0x3039`, value `x >> 16` |
| `[0x1029912c]` | read | second generator, for voices only |

## Maths

| Address | Status | What |
|---|---|---|
| `0x1005b3a0` | read | square root: power of two above, 5 Newton steps; `sqrt(0) = 1/32` |
| `0x1005b2f0` | read | arctangent, series table `0x100b1de8` (2 x 20 doubles, around 0 and 1; last term of the first has the wrong sign) |
| `0x1005b3f0` | read | `atan2(y, x)` built on it; pi `3.1415926535` (`0x100abed0`), pi/2 `1.57079632675` (`0x100abec8`) |
| `0x100a8284` | read | pi as f32 `0x40490fda`; headings `trunc((a + 2pi) / 2pi * 32 + 0.5) & 31` (`0x1002e2c4`) |
| `0x100abec0..0x100abf28` | read | doubles 1.0, pi/2, pi, 1/2pi, 0, 0.5, -pi, 1e-8, 0.4142135623, 16, 32, 6, 1e-5 |

## Tables

| Address | What |
|---|---|
| `0x100b2144` / `0x100b2148` | 9 steps (dx, dy) as int pairs: 0 east, clockwise (y down), 8 = none |
| `0x100b2130` (from `0x100b2120`) | direction from signs, index `sx * 3 + sy`: 5 4 3 6 8 2 7 0 1 |
| `0x100a8260` | neighbour order 8 0 2 4 6 1 3 5 7 (stop/snap search) |
| `0x100b218c` / `0x100b2190` | quarters of the side cells of a diagonal (stride 8): 4 8 2 1 / 2 1 4 8 |
| `0x100b222c` / `0x100b2230` | occupancy masks of the side cells (stride 8): f00 f000 f0 f / f0 f f00 f000 |
| `0x100b027c` / `0x100b0280` | occupancy added to the side cells (stride 8): 100 1000 10 1 / 10 1 100 1000 |
| `0x100b023c`, `0x100b025c` | occupancy of layer 1 (immobilised) reservations |
| `0x100b02dc` / `0x100b02e0` | marker bytes of the side cells (stride 8, minus 0x10): 4 8 2 1 / 2 1 4 8 |
| `0x100b7360` | 81 curves (5-byte records: count, pointer) from `common/CURVENEW` (loader `0x1003e1d3`) |
| `0x100b20f8` | quarter masks per direction (infantry path variant) |

## Map and cell arrays (256 x 256, index `y * 256 + x`)

| Address | What |
|---|---|
| `0x101054e6` | cell flags (u32): bits 0-3 blocked quarters, 11 ?, 14 no vehicles, 19 mine (`0x1004d740`), 20-23 mines known per player, 24-29 from a map section, 30 border, 31 checked for types with flag `0x2000000` (meaning unknown) |
| `0x101454e6` | occupancy (u32): 4-bit counters per quarter, low and high halves |
| `0x101854e6` | unit id per cell (u16), list through `unit + 0x0e` (`0x10033ed0` add, `0x10033f00` remove) |
| `0x101c54e6` | per-cell byte read each tick into `unit + 0x22` (`0x100342c0`): visibility or zones, not read |
| `0x101d54e6` | per-cell byte for flagged types in `0x1003e870` |
| `0x101e54e6` | obstacle marks for the path finder (u8), filled during the scheduler only |
| `0x101ff3c8` | per player: mask of known mines |
| `[0x100c54d4]`, `[0x100c54d8]` | map width, height in cells |
| `0x1004c980` | map loader: copies the cells, bits 24-29 and 19-23 from two sections, border `\| 0x4000000f` |

Every instruction that writes the cell flags (`grep 0x101054e6`, 199 references, checked on
2026-10-13): the loader (`0x1004cee2`); the path wrappers' temporary clear of the unit's own cell
(`0x10061f7f`, `0x10063b1f`, `0x100657b8`); mines (`0x1004d740`, bit 19); bridge states
(`0x100514b8`, `0x100514de`, `0x1005178b`, `0x10051e69`); `stand` destruction clearing its 14 x 14
footprint (`0x10054c60`, also `0x10054e98`, `0x10055640`); trees planted at run time blocking the
quarter of their trunk (`0x10055410`) and freeing it when knocked down by a type with flag
`+0x24 & 8` (`0x10055c1b`); a class (vtable around `0x100abc8c`) blocking or freeing whole cells
with bit 31 (`0x100544b6`, `0x100548bd`); a class (vtable around `0x100abe90`) clearing bit 14
(`0x10057892`); houses clearing bit 11 on 7 x 7 cells around them (`0x100505f7`). **No house code
writes the quarters**: a house's passability is the one stored in the map.

## Units (tank vtable `0x100a83f0`)

Fields (offsets from the unit):

| Offset | What |
|---|---|
| `+0x04` | unit type |
| `+0x08` | unit id (u16) |
| `+0x0a` | next in the unit list; `+0x12` next in a selection list |
| `+0x1a` | flags (u16): bit 1 dead?, 2 moving, 4 turning, `0xc000` veteran (crew boost) |
| `+0x1c` | player |
| `+0x24` | current order type (1 idle, 2 move...), `+0x25` its target, `+0x2d` order queue head |
| `+0x2f`, `+0x31` | cell x, y (i16) |
| `+0x33`, `+0x35` | offset in 1/256 px from the cell corner (i16) |
| `+0x37` | hit points |
| `+0x3d` | crew condition 0..`0x8000` (`0x10002650` averages the crew) |
| `+0x63` | hull heading 0-31, `+0x64` wanted heading, `+0x65` turn delay |
| `+0x66` | reservation: x, y (i16), direction, layer |
| `+0x6c` | path state: 0 none, 1 wanted, 2 failed, 3 ready |
| `+0x71` | ticks blocked by units (u16), `+0x73` first plan length (int, `0x7fffff` = none), `+0x77` its progress ratio (double) |
| `+0x7f`, `+0x80` | path ring index and end (64 nibbles from `+0x82`, even index = high nibble) |
| `+0xa2` | failed plans, `+0xa3` ticks before the next plan, `+0xa4` plan count |
| `+0xa8` | movement goal kind (0 none, 1 cell), `+0xa9` goal (x, y i16) |
| `+0xad`, `+0xaf` | curve point being driven to |
| `+0xb1` | distance driven (engine effects every `0x1000`) |
| `+0xb3` | immobilised (hit points under `MoveDamageHP`) |
| `+0xb4`, `+0xb5`, `+0xb6` | curve: previous direction, next direction, point index |
| `+0xb7` | driving backwards |
| `+0xb8`, `+0xbc` | weapon targets; `+0xce` turret heading, `+0xcf` turret target heading (-1 none), `+0xd0` turret delay |

Virtual methods (offset in the vtable -> address for tanks):

| Slot | Address | What |
|---|---|---|
| `+0x120` | `0x1002d290` | give a move order (resets the path when the goal changes; status 4 if current unit) |
| `+0x184` | `0x10034280` | periodic (view?) update, also called when crossing a cell area |
| `+0x188` | `0x100342c0` | per-tick cell byte read |
| `+0x1a8` | `0x10014c00` | update (crew, then `0x10010b20`) |
| `+0x1ac` | `0x1002dc60` | update when burning/dead |
| `+0xc4`, `+0xc8` | `0x10013800`, `0x1000f9f0` | order handlers: idle, move (ends on status 2/3 via `0x1002d320`) |
| `+0x264` | `0x1002da60` | marking bucket (`speed >> 3`) |
| `+0x268` | `0x1002dab0` | planning bucket (+ blocked ticks / 16) |
| `+0x26c`, `+0x270` | `0x1002cc50`, `0x1002cc60` | mark / unmark the reservation for the path finder |
| `+0x274` | `0x1002daf0` | goal cell for the path finder (-1 none) |
| `+0x278` | `0x100403a0` | plan a path (vehicle `0x10061e60`, infantry `0x100656c0`) |
| `+0x27c` | `0x10042940` | path state (`unit + 0x6c`) |
| `+0x288` | `0x10010a60` | turn hull (`0x1002e4f0` -> `0x1002ce60`), turret follows |
| `+0x294` | `0x1002d720` | stop and snap to the best neighbour cell |
| `+0x298` | `0x100142e0` | speed |
| `+0x2a0` | `0x1002deb0` | drive (`0x1002dee0`), result in `[0x100c4510]` |
| `+0x2a4` | `0x1002dee0` | driving, statuses 0 moved, 1 turning, 2 arrived, 3 failed, 4 waiting for a path |

Other unit functions: `0x10010b20` (update: `0x10017020` driving, turret turn, `0x10032950` order
dispatch on `+0x24`), `0x1002dd80` (`0x1002d590` path wait countdown), `0x1002c740` (move a
reservation: `0x1002c970` remove, `0x1002c7a0` add), `0x1002cb40` (mark), `0x1002cf40` (may enter a
neighbour cell), `0x10033f70` (change cell), `0x1002dbc0` (start a curve), `0x1002d200` /
`0x1002d332` (end the movement), `0x1004c710` (crush objects and mines when crossing a cell, not
read), `0x100326b0` (take the next queued order, `0x10032740` dispatches record types: `0x41` move,
`0x4a` attack-move, table `0x100328c4` / `0x10032870`).

## Path finding

| Address | Status | What |
|---|---|---|
| `0x100405c0` | read | per-tick scheduler (buckets, marks, plans) |
| `0x10061e60` | read | vehicle path wrapper: clears the unit's own cell, globals `0x1034d6e0` (IgnoreMines), `0x1034d6e4` (player) |
| `0x10067080` | read | planner (two searches, acceptance, waits) with callbacks |
| `0x10061f90` | read | vehicle search; tables `0x102fdd90` straight line, `0x102ddd8c` headed, `0x1031dd94` distances, `0x102add88` followed, extents `0x1033dd98`-`0x10340d98`, ring `0x10340da0`, back steps `0x103410b0` (all indexed `x * 256 + y`) |
| `0x10061d80`, `0x100636b0` | read | step check with marks |
| `0x10061c90` | read | step check with occupancy |
| `0x10061b30`, `0x10061c10` | read | clear the search tables |
| `0x10063670` | read | write a path nibble |
| `0x10061c60` | read | cut a path to n steps |
| `[0x1034d6ec]` | read | "avoid units" switch of the checks |
| `0x100639d0` / `0x10063b30`, `0x100656c0` / `0x100657d0` | seen | the other two search variants (infantry, other) |

## Unit types

Parser for vehicles `0x100479c0` (keys compared lowercased): `engine` (`+0xb2f`, `0x1004aa80`:
none/track/whell), `movespeed` (`+0xb33`, `+0xb37`), `expa_crush` (`+0xb3b`, `+0xb3f`),
`ignoremines` (`+0xb43`), `movedamagehp` (`+0xb47`), `movesmoke`, `movesmokesound`,
`canmovebackward` (`+0xb64`), `backmovespeed` (`+0xb68`, `+0xb6c`). Base class `0x100478a0` sets flag
`+0x20` bit 2 and parses `TurnDelay` (`0x100478d0`, `+0xb27`, `+0xb2b`). `GunTurnDelay` `+0xc9e`.
`pricepSpeed` towing factors `+0x1536`. Class constructors `0x10047970` (`+0x20 |= 4`),
`0x10047b80` (`+0x20 |= 8`).

## Orders and selection

| Address | Status | What |
|---|---|---|
| `0x10060bd0` | read | player move command for the selection |
| `0x100609c0` | read | group order: formation then queue an order per unit, voice `0x10060380` |
| `0x1005f1e0`, `0x1005f230`, `0x1005f290`, `0x1005f320` | read | formation (see `docs/movement.md`) |
| `0x1003e870` | read | may a group order send this unit to this cell |
| `0x10032060` | read | append an order record |
| `0x10068900` | read | rebuild a selection list from a group number |

## Map objects and drawing

Object classes (instance, vtable, class id): `dom` (7, vtable `0x100abad0`), `most` (8, `0x100abb68`), `cliff`
(`0x10288698`, `0x100abbb4`, 6), `fre` (`0x10288ab0`, `0x100abc04`, 1), `lay` (`0x10288ec0`, `0x100abc50`, 9),
`stand` (`0x1028cbb0`, `0x100abce8`, 4), `tree` (`0x10291040`, `0x100abd34`, 3), `trop` (`0x10294658`, `0x100abd80`, 2),
`vor` (`0x10298288`, `0x100abe18`, 5), `zabor` (`0x102988d8`, `0x100abe64`, 10), `unit` (`0x10294a70`, `0x100abdcc`).
Vtable entries: 0 load, 2 encode a map object, 3 decode, 4 place at a point, 5 place a map object, 12 draw. The
per-set loaders, placements and drawings are listed in `docs/formats.md` (Objects). Shared: `0x1004c260` puts an
object in its cell (record: next u16, palette nibble and 8 px quarter bits, u16 word), `0x1006ffb0` / `0x1006f740`
add a sprite to a layer's draw list (20-byte items: next, x, y, sort y, record, sprite, palette), sorted by sort y,
y, x. Layers come from `land/<SET>` (`level`), read by each class (`0x100530c0` for `cliff`'s `level1`/`level2`).
Wrecks and corpses are objects of the `unit` class (instance `0x10294a70`, vtable `0x100abdcc`, ring of 1024
records of 14 bytes at `0x10294a88`: unit type, x, y, frame; added by `0x10056360` when a vehicle is destroyed,
`0x10024f5a`, with its hull heading, or a soldier dies, `0x1001b974`). Their drawing `0x100562c0` calls the unit
type's `+0x24` (`0x100068d0` vehicles, `0x1000d0d0`) with the class's layer `+0xc`, which `0x10052390` (vtable
`+0x3c`, called by the generic `land\%s` reader `0x10052890` for every class) sets from `level`: 3 in
`land/UNIT`; the shadow goes to layer 6. Draw lists: heads at `0x1037f120 + 4 * layer`, tails at `0x1037f100`.
The map picture (redrawn only where it changed): ground, layers 0-5 (`0x1006fbf0`, `cad800.dll` `0x100057ac`, no
pixel word), shadows (`0x1006fdb0`, `0x10007938`), upright layer 7 (driver `+0xa9c8`, `cad800.dll` `0x10005e41`:
no depth test, writes `(sort y + 0x440) << 5` into the pixel word keeping bits 0-1, darkens the pixel when bit 1
is set). Every shadow item has flag 1, so bit 1 is never set and upright objects are never darkened.

Live units are not in the draw lists. Each frame (`0x100706d0`-`0x100707f9`): bit 2 of the pixel words cleared
(`+0xaa4c`), the map picture copied to the screen buffer `cad800.dll` `0x1001d588` (`+0xa9d8`, `0x10001f90`),
then three passes over the dynamic sprites (`0x10070d00`: records filed in 1024 buckets by depth + `0xa0` at
`0x1037f55c`, prepended), each calling the unit's vtable entry for the record's flag bit: `0x10070fe0` bit 1
(`+0x22c`), `0x10071190` bit 2 (`+0x230`), then `0x10070460` for bits 4 and 8 (`+0x234`, `+0x238`). A tank
(flags 7, depth = its screen y): `0x1000ee30` draws frame `128 + heading` (tracks), `0x1000ed80` the hull and
turret shadows (`+0xaa0c`, `cad800.dll` `0x10007672`: skips pixels whose word has a bit of `0x8007`, i.e. already
in a map or unit shadow, or upright objects whose depth sets bit 15; marks the word), `0x1000eeb0` the hull
(frame `heading`) and the turret. Colour sprites use `+0xa9fc` (`cad800.dll` `0x10006f08`): a pixel is drawn only
if the word pair `[pixel - 1, pixel]` is below `(depth + 0x440) << 5` in both halves, so upright objects whose sort
y reaches the unit's hide it; units are not darkened by the map's shadows. With bit 13 of `+0xdb` (bit 7 of
`+0x72` for single-sprite units) `+0xaa00` (`0x100072a2`) draws every other pixel (checkerboard on the row parity,
`0x10010054`) regardless of depth. That bit is set (`0x1000ec1b`, `0x10003c37`) when the unit is selected (bit 8
of `+0x1a`, vtable entry 0 `0x10033100`; set by the selection `0x10082edb`) or within 256 of the mouse
(`[0x103885a4]`, `[0x103885a8]` from `WM_MOUSEMOVE` at `0x10091ec4`; distance `0x10003550` on dx and 2 dy).


| Address | What |
|---|---|
| `0x10092ea0` | `.sue` archive reader |
| `0x1004f060` | player slots (4 x 0xb5 bytes at `0x101ff408`: type 1 human / 3 from `0xa`, colour, side `+3`, start group `+4`) from `XCHNG\TOGAME\mplay_info` (12 x 0x28), then `mis_players` (u32, 3 x 0x161) |
| `0x10072fc3` | interface: `intrpic.dat` (`[0x10382fc0]`), `icon.rs2` (`[0x10382b88]`), `intr.pl` |
| `0x1006f540` | the `PANL` window: rect from `intrpic.dat` frame 1 at the bottom left (`[0x10353ee4]` = screen height), minimap widget `0x10357008` |
| `0x1006e5c0`, `0x1006eef0`, `0x1006f3f2` | minimap: origin and scale (`+0x11`, `+0x15`, `+0x25`), drawing from `0x101f64e6` (preview, read by `0x1004c8b0` from `map_mini`), click to move the view |
| `0x10071e40` | initial view: middle of the map; `0x10072000` moves it to an absolute point |
| `0x100845f0`, `0x10084880` | start: zone `+0x38`/`+0x39` reset to 4/12 (`0x1004e64c`), zones handed out by `0x10086930` (random, spread by `0x10086890`), each active player's zone kept in `+0x230 + 4 * player` of the game object (vtable `0x100ac780`), view centred on the local player's zone (`[0x10387ca0]`) |
| `menu_dll.dll` `0x10017930`, `0x10018840` | splits the map into the `XCHNG\TOGAME\*` files; writes `mplay_info` from the session player list (`0x100480ec`, 0x25 bytes: name, `+0x20` side, `+0x21` colour, `+0x22` start group) |
| `0x1003fe70` | mission units: reads `XCHNG\TOGAME\mis_unitnames` (16-byte names) and `mis_mapunits`; per record `0x1003e370(name, player)` creates the unit (first free id from `player * 1024 + 1`, table `0x100b7504`, prepended to the unit list `[0x100c4508]`), then its vtable `+0x24c` applies the record (tank `0x10040190`: cell middle, then `0x10040030` builds the init block for `+0x19c`) and `+0x254` each 6-byte crew record |
| `0x10040030` | init block from a map record: byte0, byte4, HP = max(1, health% * type `+0xb8` / 100), crew condition = min(0x8000, (crew% << 15) / 100), ammo = ammo% * type `+0xbc` / `+0xc0` / 100, x, y, heading |
| `0x10033990` | base unit init: cell and offset from x, y, `+0x37` HP, `+0x39`/`+0x3b` ammo, `+0x3d` crew condition, `+0x3f` byte4, `+0x5a` byte0, 5 random bytes into `+0x46..` (draws the generator) |
| `0x100171e0` | vehicle init: also draws the generator (`+0xc0` timer); `0x1002dd30` sets `+0xb3` (immobilised) when HP < `MoveDamageHP` |
| `0x1004b54e` area | type factory: class names `tank`, `tank2`, `samo`, `atg`, `gaub`, `szenit`, `mzenit`, `apc`, `jeep`, `gruz`, `furg`, `medic`, `guner`, `art`, `para`, `crate`, `house`, `machine`, `katya` (strings `0x100b1384`-`0x100b14c0`), from `desc_common/units/UNITS` |
| `0x10043090` | tank type: reads up to `0x300` bytes of `<file>.hot` into `+0x1126` (zeroed by the constructor, so an empty file gives (0, 0) points) |
| `0x10058c20`, `0x10058d20` | mount the archives of `DataPathScm0_<i>` and `DataPathScm<climate + 1>_<i>` (`[0x10298d28]` = climate), `i` up to the first empty key; `0x10093590` splits on `;`; `0x10093090` files each entry in the 4096-slot table `0x103895e8` (32-byte entries), keeping the first one for a name unless the archive had the `^` prefix |
| `cad800.dll` `0x100044f5` | sprite RLE blitter (`0xa2` format); `0x10001c13` colour key `0xF81F` |
| `cad800.dll` `0x10008edd` | sprite dispatcher by kind: `0xa1` `0x10008f4c`, `0xa2` `0x100095a6`, `0xa3` `0x100098db`, `0xa9` `0x10009228` |
| `cad800.dll` `0x10009228` | `0xa9` blitter: palette entries are u32 (`[ebx + 4 * index]`, the others read u16), background weight `entry >> 19 & 31` (31: pixel skipped), field mask `[0x10010084]` = `G << 16 \| R \| B` (set by `0x100053d3`, `0x100067bd`), pixel = `((split(bg) * k) >> 5 & mask)` recombined + `entry & 0xffff` |
| `cad800.dll` `0x10007938`, `0x100098db` | shadow blitters: 7-bit runs, darken once per pixel; `0x10007938` (driver `+0xa9d4`, layer 6 from `Game_Dll.dll` `0x1006fdb0`) keeps the flag in the pixel word, `0x100098db` (dispatcher) tests the pixel itself |
| `0x1006fbf0` | draws layers 0-5 of the draw lists (`0x1037f120` + 4 * layer) through driver `+0xa9d0` (`cad800.dll` `0x100057ac`) |
| `cad800.dll` `0x10001130` | pixel format from the R, G, B masks (`0x100013c5`, DirectDraw surface description): field shifts, lowest bits `0x10012b38`, shadow tint `0x10012b58` = `(5 << (11 - zeros(B))) + (2 << (11 - zeros(G)))` |
| `cad800.dll` `0x101f2e50` | per-pixel word (800 x 600): depth `y << 5` in the high bits, flags in bits 0-2 and 15; `0x100026f0` fills a rectangle with depths, `0x10002790` clears bit 2, `0x10002820` adds to the depths |
| `cad800.dll` `0x10012b38`, `0x10012b40`, `0x10012b58` | lowest bit of each colour field (`0x0821`), its complement, shadow tint (`0x0085` in RGB565), set from the pixel format at `0x10001150` |
| `cad800.dll` `0x1001d49c` | table of drawing functions filled by `CADraw_Init` (`+0x28` = shadow blitter). The game reaches it through `[0x10388598]` = `cad800.dll` `0x10012af0`: its call `[drv + N]` runs the function stored at `0x10012af0 + N` (`+0xaa5c` sprite dispatcher, `+0xa9d4` shadow blitter, `+0xa9b4` ground rhomb) |
| `0x1004be20` | calls a function for each cell under a screen rectangle, row by row (screen y step 16) then x step 64; cell (cx, cy) has its rhomb's top vertex at screen ((cx - cy) * 32, (cx + cy) * 16) |
| `0x100700e0`, `0x100701a0` | ground: draws a cell at least 4 cells from the edge with its rhomb and the levels (map word >> 11) of (x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1); the 4-cell border with the empty rhomb `0x1037f158` at level 0 (black) |
| `cad800.dll` `0x10001ea0` -> `0x10003430` | rhomb rasteriser: rows 3, 7, ..., 63, 61, ..., 1 centred on the top vertex; level in 8.8 fixed point interpolated over the top triangle (cell, x + 1, y + 1) and the bottom one (y + 1, x + 1, (x + 1, y + 1)); colour from table `0x10014e5c` (`rhomb.pl` as loaded by `0x1004fac0`), palette `level` or `level + 32` in a checkerboard whose phase (`0x10010049`) flips with every drawn row; the screen is a ring buffer (`0xea600` bytes) |
| `QED.EXE` `0x415fa0` | `.ssm` writer |
| `0x100516e7` | bridge state: quarter open if grid bit `0x10 << state` |
| `0x100500c0`, `0x100511d0`, `0x10050890` | house instance, drawing (4 groups per state), state from hit points |
| `0x10054b61`, `0x10054cda`, `0x10054f02`, `0x10055694` | `stand` objects (state, footprint updates) |
| `0x100560e5` | tree state from `extra0 & 1` |
| `0x100abe64` | fence (`zabor`) class vtable: `+0x8` `0x10056e80` map object -> 5-byte cell record, `+0xc` `0x10056f60` record -> map object (kind, piece, look; see `objects::fence_frame`); `0x10056d1a` places random fences (draws the generator) |
| `[0x100c54e0]`, `0x100c54e8` | runtime object records (5 bytes: ?, ?, position quarter bits, u16 `class << 11 \| data`) and per-cell index (u16); `[0x10288664 + 4 * class]` the object classes; `0x1004d1bb` writes them back as map objects |
