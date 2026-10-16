# What the original draws for tank combat

Research for issue #6 (map #1, Tank skirmish). The question: which sprites, frames, timings and drawing passes
the original uses for a shot, a muzzle flash, an impact, an explosion, a burning or destroyed tank, the attack
reticle, the minimap's red circles, and the event the space bar centres on.

Sources: `Game_Dll.dll` and `cad800.dll` of Sudden Strike Gold (Ghidra 12.1.4 through REA, and
`llvm-objdump` for the vtable targets and hand-written blitters Ghidra does not treat as functions), and the data
in `animats.sue`, `desc_common.sue`, `desc_summer.sue` and `units.sue`. Every address below is in `Game_Dll.dll`
unless it says `cad800.dll`. **Read** means I read the disassembly or decompilation; **inferred** means a
deduction I did not check instruction by instruction; **data** means a file in the game data.

Vtables: the tank unit vtable is `0x100a8d24` (created by the tank type factory `0x10042fa0`). `docs/original-code.md`
calls `0x100a83f0` the tank vtable; another ticket found it belongs to the `apc` class. Every slot I cite here
was read from `0x100a8d24`, and I say so where it matters. The two share many slots (`+0x1ac`, `+0x1c4`,
`+0x1d0`, the drawing passes `+0x22c` to `+0x238`), but not `+0x228` (queueing the unit's sprites) or `+0x23c`.

## Short answer

- Combat visuals come from a small effect system: **animats** (`desc_common/animat/<NAME>`, a list of `.ani`
  variants with their directions and speed), placed by **explosions** (`explosions/<NAME>`, `Animat_n` lines) and
  by weapons (`shotN_Animation`, `shotN_Animat`). An effect is a 16-byte record in a pool of 8192, linked per map
  cell; it plays one frame per game tick at speed 1.0 and goes away when its last frame has played.
- **Shot in flight**: tank guns fire `tank_expl` / `tank_pierce` shots, which are of the `invis` class: no sprite
  is drawn in flight (inferred from the class name; the class's draw slot was not read).
- **Muzzle flash**: `SHOT.ANI` (192 frames, 32 directions of 6), drawn by the tank itself in its turret pass at
  the muzzle point of the `.hot` file, for 6 ticks after it fires. **Gun smoke**: `shotsmoke`, one of
  `shottsm1`/`2`/`3` at random (31 frames, 31 ticks), drawn in the last pass (`fl_smoke`).
- **Impact** on a tank: the `tank` explosion, `b_tank` = `VZRV_TNK.ANI` (23 frames, 23 ticks, `fl_smoke`). On
  the ground (summer): the `std` explosion, an additive flash `light_0` (14 frames), a fire burst `boom_d1`/`2`/`3`
  (11 frames) and a crater.
- **Destroyed tank**: light tanks use `dead_small` (flash plus `vzrvdead`, 60 frames); medium and heavy tanks use
  `dead_big` (flash plus `vzrvdeas`, 60 frames, then a self-repeating `fire` shot whose explosion plays the
  flames `flame1m`/`flame2m`, 100 or 94 frames). The tank stays on the map as a dead unit. Its sprite becomes
  frame 128 + heading (far track) under frame 96 + heading (burnt hull) and frame 224 + turret heading (burnt
  turret), in the usual unit passes. **No wreck object on layer 3 is made for a tank.**
- **Reticle**: it is the mouse cursor, itself an animat. On an enemy unit: `c9attack` (red throughout). On the
  ground: `c10agrnd` when in range (yellow for 16 frames, red for 13, yellow again: the blink), `c_a_far` when out
  of range (red throughout), `c6no` when no selected unit can do it. It animates in real time, 25 frames per
  second, looping.
- **Minimap**: an "under attack" event draws `maprred.ani` (50 frames of red rings, about 1.6 s of real time) at
  the unit's cell on the minimap, at most once per 15 s per 8 x 8-cell area. The **space bar** (`key_LA`)
  centres the view on the last minimap alert.

## The effect system

### Descriptions (data)

- `desc_common/animat/ANIMAT` lists the animat names. Each `animat/<NAME>` file has one line per variant:
  `.ani` name, number of directions, then an optional speed and two optional flags. `async 1` at the top makes the
  animat use another clock (see "Clocks"). Example: `animat/SHOTSMOKE` = `shottsm1 1`, `shottsm2 1`, `shottsm3 1`.
- An explosion (`desc_common/explosions/<NAME>`, `desc_<climate>/explosions/<NAME>` for `std`, `tree`, `howitzer`)
  has `Vor` (crater type, or `none`), `Animats n`, then `Animat_i <animat> <list> dx dy dz always`, and optional
  `AddShot`, `AddShotDmg`, `AddShotDelay`, `AddShotAccuracy`, `AddShotNum`, `AddShotRnd` (a secondary shot).
- A shot type (`desc_common/shots/<NAME>`) names the explosion per ground type: `Explosion <boomtype> <explosion>`.
  `TANK_EXPL` and `TANK_PIERCE` both give `default` → `std`, `tank_dir` / `tank_rad` → `tank`, `water` → `water`,
  `most` / `dom` → `std_solid`, `bash` → `solid_bash`, trees → `tree` / `deadtree` / `autumn`.
- A weapon (`shot1_*` keys, for example `HU/45`, `HU/76`) has `shot1_Animation shot_32` (the muzzle flash) and
  `shot1_Animat shotsmoke fl_smoke` (the smoke).

### Loading (read)

- `0x1004b130` builds an animat type for each `animat` line (`0x100408d0`, class vtable `0x100a828c`); vtable
  entry 0 (`0x10040900`) reads the `async` key into `+0x1c` and appends each variant (`0x10041500`) to the list at
  `+0x24` (count `+0x20`).
- A variant (`0x10041500`, 0x3a bytes, shared by name through the list `[0x100c4584]`): directions `+0x26`, speed
  `+0x32` = truncate(speed * 1024) (default 1024, constant `0x100a82a8` = 1024.0), two flags `+0x14` (keep loaded)
  and `+0x15` (checkerboard blitter for 16-bit animations).
- `0x10040f00` reads `<name>.ani`: a `CA` container (`u32` header whose top byte is `0xca` and whose low byte is the
  **format**, `u32` frame count into `+0x36`, then count + 1 `u32` offsets; checked on `VZRV_TNK.ANI`, 23 frames,
  offsets from byte 0x68). The format picks the drawing routine and the palette (`0x10040ca0`, read):

  | Format | Driver entry | `cad800.dll` routine | Palette | Used by |
  |---|---|---|---|---|
  | 0 | `+0xa9f8` | `0x10004dc0` (not read) | 512 bytes | none seen |
  | 1, 7 | `+0xa9fc` or `+0xaa00` (flag `+0x15`) | `0x10006f08` / `0x100072a2` | 512 bytes (RGB565) | `exbull*` |
  | 2 | `+0xaa04` | `0x10006b31` | 1024 bytes (32-bit) | almost every effect and cursor |
  | 4 | `+0xaa10` | `0x10007fcc` | 512 bytes | `light_0` |
  | 3 | (none) | custom routine `0x10040e00` | none | not seen |

  5 and 6 are refused ("Bright animats", "16bpp animats not supported yet"). The table of driver entries is filled
  by `CADraw_Init` at `cad800.dll` `0x10001ace`-`0x10001b0a`.
- Frames per direction `+0x2e` = frame count / directions. **Duration** `+0x2a` in ticks: the largest k such that
  `(k * speed) >> 10` equals the frames per direction (computed from `(n << 10) / speed` and adjusted, `0x10040f00`).
  At speed 1.0 the duration is the frame count per direction: one frame per tick.

### Placing an effect (read)

`0x100409d0` (this = animat type; arguments x, y, z, variant, direction, list, always visible):

- x, y are map coordinates in 1/32 of a cell (cell = x >> 5). Nothing is placed outside the map or when the pool is
  full (free list head `[0x1029dd80]`, `0xffff` = none). The pool is 8192 records of 16 bytes at `[0x1029dd7c]`
  (allocated by `0x100616b0`, saved with the game by `0x1008a410`), linked per cell from the u16 table
  `0x101a54e6` (256 x 256), newest first.
- Record: `+0` animat type, `+4` next (u16), `+6` and `+7` the screen offset from the cell's rhomb top vertex
  ((x & 31) - (y & 31), and ((x + y) >> 1) minus the same for the cell corner), `+8` height byte = clamp(z) / 2 +
  0x30 (z clamped to -0x60..0x19e), `+9` variant, `+0xa` direction, `+0xb` the list bit `1 << list`, plus `0x80`
  when "always visible", `+0xc` / `+0xd` cell x, y, `+0xe` start time (u16).
- The variant is chosen by the callers, uniformly with `(rng >> 16 & 0x7fff) * count >> 15` from the **second
  generator** `[0x1029912c]` (the one documented for voices): effect choice does not touch the game's generator.
  Helpers: `0x10001160` (random variant), `0x10001000` (a weapon's smoke animat and its sound), explosions
  `0x10048ff0`.

### Lists = drawing passes (read)

`0x1004ab20` maps the list names: `fl_min` and `fl_lay` 0, `fl_shadow` 1, `fl_tank` 2, `fl_gun` 3, `fl_select` 4,
`fl_smoke` and `fl_max` 5. The list bit is the bit of the per-frame dynamic sprite passes (see
`docs/original-code.md`, "Map objects and drawing"): `0x10070fe0` handles bit 1, `0x10071190` bits 2, 4, 8,
`0x10`, `0x20` in that order. For a tank (vtable `0x100a8d24`) the passes are: bit 1 `+0x22c` (`0x1000ee30`,
frame 128 + heading), bit 2 `+0x230` (`0x1000ed80`, shadows), bit 4 `+0x234` (`0x1000eeb0`, hull), bit 8 `+0x238`
(`0x1000f0e0`, turret), bit `0x10` `+0x23c` (`0x1000f240`, status bars). So:

| List | Pass | Drawn with |
|---|---|---|
| `fl_lay`, `fl_min` | 1 | before unit shadows (with a dead tank's frame 128) |
| `fl_shadow` | 2 | unit shadows |
| `fl_tank` | 4 | hulls |
| `fl_gun` | 8 | turrets |
| `fl_select` | `0x10` | status bars of selected units |
| `fl_smoke` | `0x20` | last, above every unit |

Within a pass, records come by depth bucket (screen y + 0xa0, 1024 buckets from `0x1037f55c`), so an effect is
sorted with the units of its pass by its ground point. After the last pass, shots in flight are drawn by their type's
vtable `+0x3c` (list `[0x1029dd6c]`, `0x10071190` end). `0x1008e8c0` is called between passes 2 and 4 and
`0x1008e8a0` at the end (not read).

### Drawing (read)

- Each frame, for every cell of the view, `0x10070d70` queues the cell's units (vtable `+0x228`) and then walks
  the cell's effects: `0x10040b50` returns "expired" when the elapsed time is at least the duration, and
  `0x10070d70` unlinks and frees it. Expiry happens lazily, when the cell is scanned.
- An effect is queued only if its cell is seen by the local side now (low nibble of `0x101c54e6` & `[0x10387cad]`)
  or it has the always-visible flag. Queued with flag `0x2000`, depth = screen y of its ground point, drawn at
  screen y minus z (`+8`).
- Frame = direction * frames per direction + `(elapsed * speed) >> 10` (`0x10040b50`, `0x10040960`). `0x10040960`
  calls the variant's routine with (x, y, depth, palette, frame) and marks the 16 x 8 dirty tiles (`0x10058360`,
  grid `0x10355700`).
- `cad800.dll` `0x10006b31` (format 2): rows in the `0xa1` layout (`0x00 | n` n pixels, `0x80 | n` then a byte:
  0 skips n, else repeats it), whatever the kind byte of the frame (`0xa1` or `0xa9`); colour from the 32-bit
  palette with the `0xa9` blend (background * k / 32 per field + colour, k = 31 skips); a pixel is drawn only if
  the pixel word pair is below `(depth + 0x440) << 5`, the same depth test as the unit blitter `0x10006f08`.
  So upright map objects in front hide effects as they hide units.
- `cad800.dll` `0x10007fcc` (format 4, `light_0`): the same rows, 16-bit palette, **additive with saturation per
  colour field**, same depth test. This is the flash of light under explosions.

### Clocks (read)

The start time and elapsed time use the tick counter `[0x10388434]` (25 per second at normal speed), or
`[0x10299608]` for animats with `async 1`. I found no instruction writing `[0x10299608]` by its address (it is
saved with the game, `0x1008d6d6`); it is probably a real-time or frame counter written through a pointer
(unconfirmed). `async` animats in the data: `mapack` (order markers).

## Firing a tank gun

### Muzzle flash (read)

- Not an effect record: the tank draws it in its own passes. Firing sets a countdown to 32: turret gun `+0xcd`
  (`0x1000fe17`), hull gun `+0xd1` (`0x1000fe36`); `0x10011b23` sets `+0xcd + weapon` likewise. The tank update
  (`+0x1a8` = `0x10010b20`) decrements them by one per tick (`0x10010b28`, `0x10010ba4`). Death clears both
  (`0x10010b00`).
- Turret gun, pass 8 (`0x1000f0e0`, after the turret): if the type's first weapon has an animation (type
  `+0xb74`) and `t = 32 - countdown` is below its duration, it draws frame `turret heading * frames per direction
  + (t * speed >> 10)` at the turret mount (type `+0x1126`, `.hot` points 0-31 by hull heading) plus the muzzle
  (type `+0x1226`, `.hot` points 32-63 by turret heading), at the unit's depth, with the animation's own routine
  (format 2: the alpha blitter).
- Hull gun, pass 4 (`0x1000eeb0`, after the hull): the same with type `+0xc07` (second weapon's animation), the
  countdown `+0xd1`, and the point type `+0x1326` (`.hot` points 64-95 by hull heading), skipped when the point is
  `0x7fffffff`; drawn through `0x10009d10` (not read).
- Inferred: the weapon records in the type are 0x93 bytes apart from about `+0xb70`, with the `Animation` pointer
  at `+4` (`0x10047e90` stores it at `+4` of the record, `Animat` at `+8`).
- Data: `shot_32` = `SHOT.ANI`, 192 frames, 32 directions, 6 frames each, speed 1.0: **6 ticks** (0.24 s).
  Machine guns use `shot_32m` = `MACHGFR.ani` (192 frames, 32 x 6). `SHOT.ANI` frames are 1-30 px wide.
- Inferred: the frame drawn on the tick of firing is t = 0 if the fire happens after the decrement in the same
  update (the order dispatch `0x10032950` runs at the end of `0x10010b20`).

### Gun smoke (read and data)

`0x10001000` (called from `0x1000186c`, `0x10001b6f`, `0x1001a171`, the shot creation code) plays the weapon's
sound and, if the weapon has an `Animat`, places it with `0x10001160` at the shot's starting point plus the
`dx dy dz` of the description, in its list and with its always-visible flag. For tank guns: `shotsmoke`,
`fl_smoke`, one of `SHOTTSM1/2/3.ani` (31 frames each, 1 direction): **31 ticks**.

### Shot in flight

`shots/SHOTS` gives `tank_expl` and `tank_pierce` the class `invis`; `rocket` is the other class (grenades,
rockets, `trasser`). Inferred: an `invis` shot draws nothing between gun and impact; its vtable `+0x3c` was not
read. The shot pool is 4096 records of 28 bytes at `[0x1029dd70]` (`0x100616b0`), for the firing ticket.

## Impact and explosions

### Executing an explosion (read, `0x10048ff0`)

- Nothing happens within 0x38 map units (1.75 cells) of the map edge.
- `Vor` (type `+0x1c`, `-1` = none) places a crater object (`0x10056c40`; `land/VOR`, `vor.rs2`).
- Its sound plays if the cell is seen by the local side or the explosion is flagged always audible (`+0x2c`).
- Each `Animat_i` (12-byte records from `+0x31`, count `+0x2d`: animat, list, always, dx, dy, dz) becomes an
  effect at the point plus (dx, dy, dz), random variant from the second generator, direction = the explosion's
  direction argument.
- `AddShot`: with the game generator, if `(rng >> 16 & 0xff) <= trunc(AddShotRnd * 255)` (`0x10048ec0`, constant
  `0x100aba98` = 255.0), it creates `AddShotNum` (min + random range) shots of the `AddShot` type at random angles
  around the point. This touches the game's generator and the simulation (for the damage ticket).
- Which explosion a hit uses (the `dirBoomType` / `radBoomType` of the target and the ground type) is chosen in
  `0x1003f450` (from the damage ticket, not read here).

### The explosions a tank fight uses (data)

| Explosion | Crater | Animats (list) | Frames (ticks at speed 1) | Secondary shot |
|---|---|---|---|---|
| `tank` (hit on a tank) | none | `b_tank` = `VZRV_TNK.ANI` (`fl_smoke`) | 23 | none; sound `solid` |
| `std` (summer ground, `desc_summer`) | 1 | `light_big` = `light_0` (`fl_shadow`, always visible); `b_std_fire` = `boom_d1`/`2`/`3` (`fl_gun`, offset (1, 1)); `b_std_ground` (`fl_smoke`) | 14; 11; content not decoded | none |
| `water` | none | `b_water` = `BOOM_WF` (`fl_smoke`, always) | not counted | none, `NoDamage 1` |
| `std_solid` (bridge, house) | none | `light_big` (`fl_gun`, always); `b_most_fire` = `vzrvmst1/3/4/5` (`fl_smoke`); `b_most_smoke` = `vzsmmst2/3` (`fl_select`, z 64) | 14; 60/36/32/59; 59/53 | `fire`, 1-3 |
| `dead_small` (light tanks die) | 4 | `light_big` (`fl_shadow`); `dead_2` = `vzrvdead.ani` (`fl_smoke`) | 14; 60 | `invis`, damage 100, 1 shot, chance 0.45 |
| `dead_big` (medium and heavy tanks die) | 3 | `light_big` (`fl_shadow`); `dead_1` = `vzrvdeas.ani` (`fl_smoke`) | 14; 60 | `fire`, damage 200, 1 shot, chance 0.9 |
| `fire` | 5 | `fire` = `FLAME1M` / `FLAME2M` (`fl_smoke`) | 100 / 94 | `fire`, damage 12, delay 25-30, 1-2 shots, chance 0.9 |

`DieExplosion` comes from `HU/LIGHT.TANK.COMMON` (`dead_small`), `HU/MEDIUM.TANK.COMMON` and
`HU/HEAVY.TANK.COMMON` (`dead_big`). `b_std_ground` (`COMMON/animat/B_STD_GROUND`, included by the summer file)
came out of `tools/sue_extract.py` as unreadable bytes, so its `.ani` names are unknown (open question). The winter
and desert `std` were not checked.

### Burning (data, inferred)

There is no "burning" sprite on the tank. A medium or heavy tank's `dead_big` fires one `fire` shot whose
explosion plays a 4-second flame and fires another `fire` shot 25-30 ticks later with chance 230/256, and so on:
a chain of flames on and around the wreck (expected length about 10 links, inferred from the chance). Light tanks
(`dead_small`) do not start this chain.

## The destroyed tank

### What happens (read)

- Death: tank `+0x1b4` = `0x10010b00` clears the gun countdowns and calls `0x1002dcb0`, which calls the base
  `0x10033690` (dead flag: bit 0 of `+0x1a`; `DieExplosion` type `+0x2f4` at the unit's pixel through
  `0x10048ff0`, unless flag 4; `DieSound` `+0x2f8`; the crew roll; **hit points reset to the type's maximum**
  `+0xb8`) and sets `+0xb3` = 1.
- The dead tank stays a unit. Its update `+0x1ac` (`0x1002dc60`) draws the game generator every tick and, when bit
  16 is set, draws it again and subtracts bit 0 from the hit points: one point lost with probability 1/4 per tick.
  Hits on a dead unit take half their damage (`0x10032c40`). At 0 the unit is removed (`0x1003ee50`).
- No wreck object: the wreck ring at `0x10294a88` (layer 3, `0x10056360`) is filled by `0x10024f00`, which is
  slot `+0x1b4` of the vtables `0x100a9bc8`, `0x100a9eb4`, `0x100aa1e4` (constructors `0x100442b4`, `0x10044394`,
  `0x10044ab4`: other vehicle classes, not identified), and by soldiers (`0x1001b974`). Not by tanks.

### Its sprite (read)

The tank's queue routine `+0x228` = `0x1000eaf0` builds the draw state at `+0xd2`: screen x `+0xd2`, screen y
`+0xd4` (`-32000` when the local side does not see it: then nothing is queued), and the u16 `+0xdb`: bits 0-4
turret heading (`+0xce`), bits 6-10 hull heading (`+0x63`), bits 11-12 the hull state (`0x1000` if dead, else
`+0xb3 & 3`), bit 13 the checkerboard (selected or within 256 of the mouse). The record's pass flags are `0xe`
(passes 2, 4, 8) and `0xf` when dead (adding pass 1); `+0x10` when selected (`0x1000ed02`, `0x1000ed49`).
This corrects "flags 7" in `docs/original-code.md`: pass 1 is used only by a dead tank.

Frame tables of the type (288 frame pointers from `+0xca6`, loaded by `0x10043183`): `+0xca6` frame 0, `+0xd26`
frame 32, `+0xe26` frame 96, `+0xea6` frame 128, `+0xf26` frame 160, `+0xfa6` frame 192, `+0x10a6` frame 256.

| State | Pass 1 (`0x1000ee30`) | Pass 4 (`0x1000eeb0`) | Pass 8 (`0x1000f0e0`) |
|---|---|---|---|
| alive (`+0xb3` = 0) | not drawn | frame `h`, then frame `32 + h` over it | frame `192 + t` at the mount |
| immobilised (`+0xb3` = 1) | not drawn | frame `h`, then frame `64 + h` | frame `192 + t` |
| dead | frame `128 + h` | frame `96 + h` only | frame `224 + t` |

(h = hull heading, t = turret heading.) Rendered from `units.sue` `t-26.pck` for this note: frames 32-63 and 64-95
are the near track and running gear (two versions, the second inferred to be the damaged track), 96-127 a hull
without turret, 128-159 the far track, 224-255 a dark burnt turret. This refines the frame table in
`docs/formats.md` ("hull variants (damage states? to check)").

### Blink of an ordered unit (read)

Bits 10-13 of `+0x1a` are a countdown set to 15 for the units that take a player's order (`0x10061121`,
`0x10060882`, `0x1005ff25`; the group order code `|= 0x3c` in byte `+0x1b`), decremented once per tick
(`0x1003e4e0`, called from the tick at `0x10090d52`). While it runs, the queued record's `+0xe` is
`(count / 3) & 1`, which selects the second 512-byte palette of the type (type `+0x1426`, two palettes allocated
at `0x10043122`): the unit flashes every 3 ticks for 15 ticks. Inferred: the second palette is a brightened copy
made at load.

### Order markers (read)

A local player's orders also leave a marker at the target: the `mapack` animat (`misc/INTERFACE` key `mapack`,
`[0x10382980]`; variants `c_move`, `c_a_land`; `async`), lists from `mapacklevel` (`[0x10382940]`, all
`fl_smoke`), always visible: variant 0 for moves (`0x10061160`, `0x10061240`; `0x10060920` with a variant
argument), variant 1 (`c_a_land`) for attacks (`0x10060f00`, `0x10060fd0`).

## The attack reticle (read)

- It is the mouse cursor. `misc/mouse` maps cursor names to animats: `AttackUnit m_attackunit`,
  `AttackPoint m_attackpoint`. `0x100735d0` / `0x10073e50` load them into the table `0x10386ff0` (attack unit id 6,
  attack point id `0xe`, move `0xd`, attack-move `0x10`).
- `m_attackunit`: variant 0 `c6no`, 1 `c9attack` at speed 0.5, 2 `c9attack`. `m_attackpoint`: 0 `c6no`, 1 `c_a_far`
  at 0.5, 2 `c10agrnd`. All are 31-frame, one-direction, 24 x 13 (`c6no` 26 x 21) format-2 sprites.
- Choice (`0x10080580`, when an order is armed, `[0x103880b8]` = 3, switch on the order): for attack,
  `0x10083b30(unit under the mouse)` asks the target `+0x8c(local player)` (tank `0x10033490`) and returns the
  best answer of the local player's selected live units to `+0x24(target)` (tank `0x10010550`); a non-zero answer
  gives cursor 6 with that variant. Otherwise `0x10083bb0(x, y)` returns the best `+0x28(x, y)` (tank
  `0x100108b0`) and the cursor is `0xe` with that variant. Inferred from `0x100108b0`: 0 = cannot (no
  `CanAttackPoint`, no ammunition, inside the dead zone, immobilised), 1 = out of range, 2 = in range, with the
  ranges scaled by the crew condition; the exact rule belongs to the target-acquisition ticket.
- Colours (data, average of each frame's opaque pixels): `c10agrnd` frames 0-15 yellow (red and green), 16-28 red,
  29-30 yellow again: the yellow-red blink. `c_a_far` red in every frame. `c9attack` red in every frame. So:
  ground in range blinks yellow and red, ground out of range stays red, a unit is always red; out of range on a unit
  pulses at half speed (variant 1).
- Timing (`0x10073740`): frame = `((now_ms - start) / 40 mod duration) * speed >> 10`, real time (`timeGetTime`),
  looping: 25 frames per second whatever the game speed; `c10agrnd` loops every 31 frames (1.24 s), `c_a_far` and
  the slow `c9attack` every 62 (2.48 s). Inferred: the cursor is drawn through the sprite dispatcher (not
  checked).

## The minimap's red circles and the space bar (read)

- **Trigger**: the hit handler `+0x1d0` (`0x10032c40`, same slot in both vtables) sends event `0x80` to the hit unit
  (`+0x1c4`, `0x10033b80`) when its last hit (`+0x42`, tick) is more than 300 ticks old and the attacker's player is
  neither the unit's own nor player 12. The unit forwards it to `0x10082630(0x80, cell x, cell y)` if it belongs
  to the local player.
- **Throttle**: `0x10082630` keeps a millisecond time stamp per 8 x 8-cell block (32 x 32 u32 at `0x101f54e6`).
  If the block's stamp is older than `UnderAttackInterval` * 1000 ms (`misc/INTERFACE`: 15, `[0x100b2404]`), it
  plays the nation's "under attack" voice (`0x1006bc90`) and adds a minimap alert (`0x1006e900(x, y, 1)`). In
  every case it stamps the 3 x 3 blocks around with the current time, so a continuous attack keeps the alert quiet.
- **Alert**: `0x1006e900` projects the cell to the minimap ((x - y) >> scale, (x + y) / 2 >> scale) and
  `0x1006e860` appends a 24-byte record (next, minimap x, y, variant clamped to 0-2, start time in ms, frame -1)
  to the widget's list `+0x26`. Its animat is `miniani` (`[0x10382984]`): variant 0 `maprgree`, 1 `maprred`
  (both 50 frames of `0xa9` sprites up to 138 x 80). Green alerts come from `0x1007676a` and `0x10079064`
  (callers not identified; inferred to be mission messages).
- **Animation**: the widget's update (vtable `0x100abfd0` entry 3, `0x1006e930`) sets the frame to
  `((now_ms - start) * speed) >> 15`, i.e. one frame per 32 ms at speed 1.0, invalidates the old and new frame
  rectangles (`0x1006e740`), and frees the record after the last frame: **50 frames, 1.6 s of real time**. The
  minimap draw (`0x1006eb30`) draws each alert after the units, clipped to the widget (`0x1006ee40`), through the
  sprite dispatcher `+0xaa5c` (`cad800.dll` `0x10008edd`, kind `0xa9` → `0x10009228`).
- **Space bar**: `0x1006e860` also stores the alert's position in view coordinates, `[0x103828bc]` = minimap x <<
  (scale + 5) = (x - y) * 32 and `[0x103828b8]` = (x + y) * 16 (the cell's rhomb top vertex, minus the minimap's
  rounding), for every alert, red or green. The view init sets both to `0x7fffffff` (`0x10071f19`). The key
  handler (`0x10081d5a`) compares the key with the `key_LA` entry of the key table (`[0x1029926c]`, name `la` at
  index 15 of `0x100b1b7c`; default `20 Space` in `key.map`) and, when a position is stored, moves the view to it
  minus half the screen (`0x10072000`). So the space bar centres on the **last minimap alert**, usually the last
  "under attack" event.

## Other effects seen on the way

- Moving vehicles leave engine smoke: `MoveSmoke engine_1 fl_tank` (light tanks, `ZAVGRUZ.ANI`) or `engine_2`
  (medium and heavy, `ZAVTANK.ANI`), 480 frames, 32 directions of 15. `0x10029bc0` places it at the unit's pixel
  plus the type's offsets (`+0x1b82`, `+0x1b84`, `+0x1b86`) plus a jitter of `(rng >> 16 & 15) - 7` drawn from the
  **game generator** (twice), direction `(hull heading - 16) & 31`. The class this function belongs to was not
  checked. If tanks use it, engine smoke changes the simulation's random sequence (for the movement notes).
- Inferred: shells ejected by machine guns (`SHELLS` = `machgshb`, 32 directions) and tracer `TRASSER` belong to
  other weapons; not used by the tank guns of the reference maps.

## Facts for the other tickets

- Damage: `0x10032c40` (hit handler, `+0x1d0`): damage from `+0x1cc`, applied through `+0x1d8`; on a dead unit,
  hit points minus half the damage. `0x10010b20`: every 64 ticks (when `tick & 63 == id & 63`), a tank with
  0 < hit points < max / 4 calls `0x1003ef50(unit, -1, 0, 12)` (vtable `+0x1d4`), inferred to be self-damage from
  fire. Burning: `0x1002dc60` (above).
- Group attack orders (`0x10060fd0`, `0x10061160`, `0x10061240`) send the order to a single unit, the one with the
  lowest cost (`0x10029e70`, `0x100208c0`, `0x100209e0`), not to every selected unit (read, not followed further).
- `AddShot` chains use the game generator (`0x10048ff0`); effect variants use the second generator.

## Open questions

- `b_std_ground`: the included file `COMMON/animat/B_STD_GROUND` did not extract as text.
- The writer of the `async` clock `[0x10299608]`.
- The `invis` shot class's draw slot (`+0x3c`), to confirm no sprite in flight.
- `0x10009d10` (hull gun flash) and `cad800.dll` `0x10004dc0` (format 0) were not read.
- The exact meaning of the cursor scores 1 and 2 from `0x10010550` and `0x100108b0`.
- Which vehicle classes leave a layer-3 wreck (`0x10024f00`), and which class `0x10029bc0` (engine smoke) serves.
