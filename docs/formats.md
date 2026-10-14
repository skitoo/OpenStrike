# Sudden Strike Gold file formats

Analysed version: Sudden Strike Gold (Steam), 510 MB.

## Binaries

| File | Size | Role (hypothesis) |
|---|---|---|
| `GAME_EXE.EXE` | 73 KB | Launcher (PE32 i386) |
| `Game_Dll.dll` | 807 KB | Game engine and logic: **main RE target** |
| `menu_dll.dll` | 307 KB | Menus |
| `scridll.dll` | 70 KB | Mission scripts |
| `text_dll.dll` | 20 KB | Text |
| `cad640/800/1024.dll` | ~95 KB | Per-resolution layouts |
| `binkw32.dll` | 286 KB | Bink video (RAD) |
| `QED.EXE` | 782 KB | Map editor |

### `Game_Dll.dll` (Ghidra 12.1.4)

- x86 PE DLL, image base `0x10000000`, SHA-256 `38f1cdb6...ab19`.
- 2,271 functions, 821 strings.
- `.text` 668 KB, `.rdata` 32 KB, `.data` ~3.1 MB (large static tables).

## `.sue` archives (decoded)

Reader: `Game_Dll.dll` `0x10092ea0` (opens an archive, checks the `FZFS` magic `0x53465a46`, loads the directory).
Directory blocks are read by `0x10093010`; entries are inserted into a 4096-slot hash table by `0x10093090`.
Up to 16 archives can be mounted. A `^` prefix on the path forces overriding entries already present.

**Which archives are mounted.** The keys of `SudTest.ini` (`[Game]`, names from `text_dll.dll` string 44
`DataPathScm%i_%i`): `DataPathScm0_<i>` at start-up (`0x10058c20`), then `DataPathScm<climate + 1>_<i>` for a map
(`0x10058d20`, climate = map header byte 4: summer 0, winter 1, beach 2, desert 3), for `i` = 0, 1, ... up to the
first missing or empty key (32 at most); each value is a `;`-separated list mounted in order by `0x10093590`. A
name already in the hash table keeps its first entry (unless the archive has the `^` prefix), so **the first archive
listed wins** (beach: `beach.sue` before `summer.sue` for `mappic.rs2` and `rhomb.pl`; `units.sue` before
`animats.sue` for `str.col`) and the `_1` keys of the shipped `SudTest.ini`, which mount `common.sue`, `summer.sue`
and `winter.sue` again, change nothing (`DataPathScm3_2` is never read, `3_1` being absent). `menu.sue` comes from
`MenuResource`, the briefings from `BriefingsDir`. **`common01.sue`, `summer01.sue`, `winter01.sue` and
`menu01.sue` are named by no key and never opened**, neither by the game nor by the editor (`editor2.ini` lists
`desc_*`, `common`, `units_edit`, the climate and `*_edit` archives): they are older copies of the same data
(unit descriptions written out instead of `#include`d, 206 of the 446 `common01` files differ from `common`).
OpenStrike mounts from `SudTest.ini` the same way (`crates/formats/src/ini.rs`, `crates/client/src/assets.rs`).
Extractor: `tools/sue_extract.py`, verified on all 24 archives (2,943 files, sizes checked).

```
"FZFS"  u32 dir_off                  directory at file offset 8 + dir_off (end of the file)
directory = 3 blocks, each:
    u32 comp_size, u32 count, u32 elem_size, then comp_size bytes of zlib (count * elem_size once inflated)
  1. entries, 20 bytes each:
       u32 name_off     offset into the names block
       u32 base         always 8 (meaning not confirmed)
       u32 first_chunk  index into the offsets table
       u32 size         uncompressed size
       u32 chunk_size   0x4000
  2. names: NUL-terminated strings, `\` as the path separator (`units\t-26`)
  3. offsets: u32 absolute file offsets; chunk i spans offsets[i]..offsets[i+1]
data = for each file, ceil(size / chunk_size) consecutive chunks, each zlib-compressed
       (stored raw when the raw length equals the expected length)
```

The file-reading function itself has not been decompiled yet: the "raw stored chunk" case is inferred, not observed in the code.

### Contents

- `.col` (519) palettes and `.pck` (376) sprites: see below. `.ani` (229), `.hot` (123), `.rs2` (98): to analyse.
- Files with no extension (1,182), mostly in `desc_*`: **plain-text unit descriptions**. Example `desc_common/units/t-26`:
  armour by face (`front=15 side=15 turret=15`), gun calibre, ammo, speed, health,
  `#include`-style inheritance (`#hu\light.tank.common`), tab-separated key/value pairs.
- `.txt`, `.scc`, `.apm`, `.edt`, `.seq`: scripts, sequences, editor data.

## Palettes `.col`

256 little-endian RGB565 colours (512 bytes).

## Sprites `.pck`

Decoder: `tools/pck.py`, check: `tools/pck_validate.py`.
Validated on 280 files / 36,323 frames (every row ends on its length and every frame on its boundary).
The blitter lives in `cad800.dll` (and its `cad640`/`cad1024` siblings), a table of ~60 drawing routines
returned by its only export `CADraw_Init`; the sprite routine is hand-written assembly at `0x100044f5`.

```
file  = u32 offsets[n + 1] from byte 0           (frame i = offsets[i]..offsets[i + 1])
      | u32 header, then the same table at byte 4 (PARA, SHOT, STR...)
frame = i16 x0, i16 y0      anchor offset (draw at position + (x0, y0))
        u16 width, u16 height
        u8  kind            0xa1 / 0xa2 / 0xa9 colour, 0xa3 shadow, 0xa5 unknown
        rows[height]: u16 byte length, then commands
```

The drawing dispatcher (`cad800.dll` `0x10008edd`) picks the row format from the kind. Checked on every sprite of
the data: each kind's rows end exactly on their byte length with its own format, and only with it.

| Kind | Blitter | Commands |
|---|---|---|
| `0xa2` (all unit sprites, ground sprites) | `0x100095a6`, also `0x100044f5` | 2-bit mode, 6-bit n: `0x00` copy n palette pixels; `0x40` copy n pixels blended 50 % with the background, `(src + dst - ((src ^ dst) & mask)) >> 1`; `0x80` repeat the next palette pixel n times; `0xC0` skip n |
| `0xa1` (upright sprites of map objects) | `0x10008f4c` | 7-bit n: `0x00` copy n palette pixels; `0x80`, then a palette index c: c = 0 skips n, else repeats c n times |
| `0xa9` | `0x10009228` | as `0xa1`, with the 32-bit palettes of the effects (`animats/*.col`, 1024 bytes): low 16 bits an RGB565 colour already weighted, bits 19-23 the background weight `k`; pixel = background * k / 32 per field (mask `G << 16 \| R \| B`, `0x07e0f81f`) + colour, `k = 31` leaves it untouched. Used only by effects: 4125 frames in 87 `animats/*.ani` (muzzle fire, explosions, dust, splashes) and `SHOT.PCK` |

Reading `0xa1` sprites with the `0xa2` format (as OpenStrike did before) cuts rows apart: bits of ships and cranes
float beside them.

Shadow frames (`0xa3`) use 1-bit modes with 7-bit counts: `0x00 | n` is n shadow pixels (n up to 127), `0x80 | n`
skips n pixels. The map's shadow layer (6) is drawn by `Game_Dll.dll` `0x1006fdb0` through the driver entry
`+0xa9d4`, i.e. `cad800.dll` `0x10007938` (the other layers use `+0xa9d0`, `0x100057ac`). Each screen pixel has a
16-bit word (buffer `0x101f2e50` = back buffer `0x101081ec` + `0xeac64`, 800 x 600, filled with the row's depth
`y << 5`): a shadow pixel is drawn only if the word has none of bits `0x8007`, then the shadow's flag is set in it
and the pixel becomes `((p & 0xf7de) >> 1) + 0x0085` in RGB565 (half brightness, +4/63 green, +5/31 blue). So
overlapping shadows darken once. The flag is the draw item's `+0x10`, 1 for every shadow (map objects and units).
Upright sprites (layer 7, `0x10005e41`) drawn on a pixel whose word has bit 1 set would be darkened the same way,
but no shadow sets bit 1, so they never are. Units are drawn later, on each frame, and are not darkened by the
map's shadows; their own shadows (`0x10007672`) skip pixels already shadowed and upright objects (see
`docs/original-code.md`, Map objects and drawing).

The tint comes from the screen's pixel format (`0x100013c5` reads the DirectDraw surface description and calls
`0x10001130(R mask, G mask, B mask)`): `(5 << (11 - leading zeros of B)) + (2 << (11 - leading zeros of G))`, the
lowest-bit mask `0x0821` and its complement `0xf7de` likewise. In RGB565 that is `0x0085`; in RGB555 `0x0045`, the
same proportions. `cad640.dll` and `cad1024.dll` compute the same. The dispatcher's own `0xa3` path (`0x100098db`)
tests the pixel itself instead of the word and is not used for the map's shadows.

A screenshot found on the Internet (version unknown) shows neutral shadows, about 0.64 times the lit colour;
OpenStrike drew that for a while, but follows the code now.

Unit files hold the colour frames first, then the shadows (BRON: 288 frames, shadows from 160, 64 of them).

Not handled yet:
- `xx 00 00 CA` container (47 files: briefing backgrounds 640x480, menu animations, `d_shadow`);
- kind `0xa5` (258 frames, menus);
- `PARA.PCK` (header 21, table of 43 offsets) and menu fonts. `SHOT.PCK` and `STR.PCK` (u32 frame count, then
  count + 1 offsets) are handled.
- `.hot` (hotspots?) and `.ani` (animations): to analyse.

## Vehicle sprite layout and `.hot` attachment points

Observed on `t-26.pck` (288 frames) with `t-26.hot` (64 points):

| Frames | Content |
|---|---|
| 0-31 | hull, 32 headings |
| 32-127 | hull variants (damage states? to check) |
| 128-159 | tracks and running gear, drawn before the shadows (`Game_Dll.dll` `0x1000ee30`, type table `+0xea6`) |
| 160-191 | hull shadows (first `0xa3` run) |
| 192-223 | turret, 32 headings |
| 224-255 | turret variants (to check) |
| 256-287 | turret shadows (second `0xa3` run) |

`.hot` = i32 (x, y) pairs. For the T-26: points 0-31 are the turret mount per hull frame (about (0, -13)),
points 32-63 the gun muzzle per turret frame (they trace an ellipse). **Frame k faces map heading k** (0 the map's
east, clockwise in 11.25 degree steps): the game draws the hull frame `heading & 31` and the turret frame
`turret heading & 31` (`Game_Dll.dll` `0x1000eb7b`, `0x1000eb99`), with no projection.
Decoder: `crates/formats/src/hot.rs`. Other vehicles have more points (`bron.hot`: 128), meaning unknown.

## Maps

Three gzip-compressed formats: `SSSM` single-player maps (`.ssm`, version `0x10f`, handled),
`SSMM` multiplayer maps (`.smm`, version `0x112`) and `CAMP` campaigns (`.ssc`, embed their maps): not handled yet.

### `.ssm` (single player)

Layout from the map **writer** in the editor, `QED.EXE` `0x415fa0`, which writes 13 sections in order.
Parser: `tools/ssm_probe.py`, validated up to the end of the object list on all 164 single-player maps.

```
"SSSM", u32 version (0x10f), 16-byte GUID
u32 x5 (2, 12, 2, 4, 0 in the samples: meaning unknown), u32 width, u32 height   (cells, 128 or 256)
briefing:  u32 length including the NUL, then the text (latin-1)               0x415790
preview:   height x width RGB565 (half size when a side is > 128)              0x414610
ground:    width x height u16 = palette_select << 11 | rhomb index             0x414710
           width x height u32 second layer (flags / heights? unknown)
           u32 11, then 11 x 16-byte object set names:
             bred fre lay stand tree trop dom zabor cliff vor most             0x414210
           u32 n, n x 8-byte objects: i16 x, i16 y (map pixels, 32 per cell),
             u8 set (index into the names above), u8 frame, u8 ?, u8 ?
then (not parsed yet): u32 n + n x 16 bytes (0x415140); 100 x 27-byte records (0x4151b0);
units, size-prefixed (0x4152f0); 3-4 player/side records of 0x161 bytes (0x415b10);
0x4e-byte records + a byte grid (0x4155d0); compressed blobs (`VOVA_Comp`, 0x4156b0);
a byte grid (0x415740); two size-prefixed blocks (0x415a70, 0x415cb0); 2x i16 pairs (0x415d50)
```

### Ground

Loader: `Game_Dll.dll` `0x1004fac0` (loads `rhomb.pl` and `mappic.rs2`, keeps the used rhombs, 0x400 bytes each).

- `mappic.rs2` (one per climate archive): raw 8-bit **rhombs** of 1024 bytes, each a 64x32 diamond whose rows
  are 3, 7, ..., 63, 61, ..., 1 pixels wide, every row centred on the top vertex (`cad800.dll` `0x10003430`).
- `rhomb.pl`: 64 palettes of 256 RGB565 colours: two ramps of 32 **brightness levels** (0 dark, 31 bright),
  palettes 0-31 and 32-63, the second slightly brighter. The 5-bit level of each cell (16 almost everywhere on
  the sample map, 31 on a few cells) is the level at the rhomb's top vertex: the game interpolates the level of
  each pixel between the cell, the cells at x + 1 (right vertex), y + 1 (left vertex) and (x + 1, y + 1) (bottom
  vertex), and alternates palettes `level` and `level + 32` in a checkerboard.
- Cells within 4 cells of the map edge are drawn black (`Game_Dll.dll` `0x100701a0`).
- **Isometric projection** (inferred, see below): map pixel (x, y) is drawn at screen (x - y, (x + y) / 2), so a
  32x32 cell becomes a 64x32 rhomb, and cell (cx, cy) is centred at ((cx - cy) * 32, (cx + cy) * 16 + 16).
  Evidence: rail pieces (`trop` frame 48, a diagonal 140x68 sprite) are placed at a constant x every 80 map pixels;
  they only join into a track under this projection. The map is a diamond on screen.
- Code: `crates/formats/src/iso.rs`, `crates/formats/src/rhomb.rs`.

### Second layer: passability and ground types

`layer2` (u32 per cell):

- **Bits 0-3: the game's passability**, one bit per 16 px quarter of the cell, bit `(tx & 1) + 2 * (ty & 1)`,
  1 = blocked. The editor stores it with water, houses and intact bridges already applied: all water cells are 0xF,
  every solid house tile is set, and 98.9 % of the bridge tiles of 60 maps match the bridge rule below.
  The game rewrites these bits when a bridge changes state (`Game_Dll.dll` `0x100516e7`) or a house is destroyed
  (`0x10054cda`, `0x10054f02`, `0x10055694`). No house code writes them at run time (every writer is listed in
  `docs/original-code.md`): the stored bits are the houses' passability. OpenStrike reads them as its terrain.
- Bits 8-11: the ground type, in the order of `desc_common/GROUNDTYPES`: grass, lightgrass, pashnya (ploughed), swamp,
  darkground, lightground, **water** (6), sand.
- Bit 15 and bits 16+: not understood yet.

### Object states

`Object::extra[0]` is the object's state in the map (0 = intact). Seen on all single-player maps: houses 0-3
(1,649 damaged), trees 0-1 (3,092 down), `stand` 0-1, `lay` 0-1, bridges 0-3. Fences (`zabor`) use both bytes for the frame: `extra[0]` is the
fence kind (blocks of 40 frames), `extra[1]` bits 1-3 the piece (4 = post) and bit 0 an alternative look; the game
rebuilds the frame from them (`Game_Dll.dll` `0x10056e80` / `0x10056f60`, `objects::fence_frame`, checked on all
190,284 fences). Bridges, houses, trees and `stand` objects are drawn in their stored state (`objects::map_state`); other sets are
drawn intact.

Rules read in `Game_Dll.dll`:
- `stand` (record builder `0x10054b61`): `extra[0] != 0` gives state 1; the draw routine (`0x100551a0`) uses slot
  `state` for the colour sprite (layer 7), `2 + state` for the shadow (layer 6), `4 + state` for the ground sprite.
  `stand` frames also carry a 14x14 footprint grid at byte 9 (before the slot table at 205); the routines
  `0x10054cda`, `0x10054f02`, `0x10055694` that test bit 0x01 with origin 6 belong to this class, not to houses.
- Trees (builder `0x100560e5`, draw `0x10055e01`): state = `extra[0] & 1`, same slot rule from byte 5.
- Houses: each placed house becomes an instance (`0x100500c0`, called from `0x10050dae`) with type = object frame and
  state = `extra[0]` (0-3); the draw routine (`0x100511d0`) takes per state four sprites (left half, right half,
  ground, shadow). In the files, every one of the 113 summer houses has exactly 4 sprite groups ending with a shadow,
  one per state (some groups lack a half, or are only ground + shadow: a ruin). At run time the state follows the hit points (`0x10050890`: thresholds from the HP values of
  `land/dom`, one per state). The house footprint rule (bit 0x01, origin 6) is confirmed by the data (all 4,245
  solid tiles are blocked in the map) but its routine has not been read yet.

### Water

Water cells use rhombs whose pixels sit in palette indices 146-156, which are dark green in every `rhomb.pl`
palette: the in-game water is most likely that dark green (the minimap draws it blue).
Waves are animated overlays: `desc_common/misc/WAVES` (`wave water 8 wolny fl_lay ...`) and
`animat/WOLNY` (`woln1`, `woln2`, `woln3`, animations in `animats/woln*.ani` + `.col`). Not rendered yet.

### Objects

Object sets are `<name>.rs2` + `<name>.pl` in the climate archive; `dom` is `houses.rs2`. `.rs2` = u32 count, u32
offsets[count + 1], then one frame per object (read the same way by the game, `Game_Dll.dll` `0x10059250`). Each set
has a class in the game: its vtable's entry 0 loads the frames, entry 2 packs a map object into a 5-byte cell record
(`frame & 0x7f` and the state bits in a u16 with the class id in bits 11-15), entry 5 places it, entry 12 draws it.
Every set below is decoded as its loader reads it; on the three climates every sprite fills exactly the room the
layout gives it (9 desert `stand` sprites have rows a little too long, which the blitter just cuts), and every object
of the 164 single-player maps finds its sprites.

| Set | Class (load, place, draw) | Frame | Parts and layers | Placed at | State |
|---|---|---|---|---|---|
| `cliff` | `0x10052dc0`, `0x10052f50`, `0x10053010` | 2 u32 offsets (0 = none) | both on layers `level1`, `level2` (0) | cell corner | none |
| `trop` | `0x10053850`, `0x100566d0`, `0x10053730` | the sprite | layer 1 | 8 px square middle | none |
| `fre` | same as `trop` | the sprite | layer 2 | 8 px square middle | none |
| `vor` | `0x10056510`, `0x100566d0`, `0x10056a10` | the sprite | layer 3, only where the player sees | 8 px square middle | none |
| `lay` | `0x100539f0`, `0x10053cd0`, `0x10053fc0` | u8 has-second, u32 len, sprite, [u32 len, sprite] | layer 4 | 16 px quarter middle | `extra[0] & 1` |
| `tree` | `0x100557e0`, `0x100559d0`, `0x10055d80` | 5 bytes, 6-slot table | upright 7, shadow 6, ground 3 | 16 px quarter middle | `extra[0] & 1` |
| `stand` | `0x10054950`, `0x100559d0`, `0x100551a0` | u32 palette index (5 palettes), ..., footprint at 9, 6-slot table at 205 | upright 7, shadow 6, ground 2 | 16 px quarter middle | destroyed if `extra[0]` |
| `zabor` | `0x10056dc0`, `0x10057070`, `0x10057310` | u32 len, upright sprite, shadow sprite | upright 7 (flat look: layer 5), shadow 6 | cell middle | see fences |
| `dom` | `0x10050980`, `0x10050d40`, `0x100511d0` | see houses | halves 7 (+ depth), ground 0, shadow 6 | cell corner | `extra[0] & 3` |
| `most` | `0x10052040`, -, `0x10052450` | see bridges | abutments and deck 0, shadows 6 | as stored | `extra[0] & 3` |

Positions: the cell record keeps the cell and an 8 px quarter (`0x1004c260`); `trop`, `fre` and `vor` are drawn 4 px
lower on screen, i.e. at the middle of their 8 px square. Houses, trees, `stand` and fences are dropped within 4
cells of the map edge (their placement routines), the others only outside the map. Layers (`land/<SET>`, `level`,
the same in every climate) are drawn in order 0 to 7, each sorted by sort y, then y, then x (`0x1006f740`); 6 holds
the shadows and 7 the upright sprites. Palettes: the set's `.pl` (one palette; `stand.pl` holds 5 and the frame
picks one), or the frame's own for `dom` and `most`.

The **6-slot table** (u32 offsets in the frame, 0 = none) is: upright intact, upright destroyed, shadow intact,
shadow destroyed, ground intact, ground destroyed. A tree's "ground destroyed" is the fallen tree; a sunken barge is
an empty upright sprite, a shadow and a ground sprite.

**Houses** (`dom`, loader `0x10050980`): the 512-byte palette, a u16, a u32, the **14x14 footprint grid** (16 px
tiles, grid tile (6, 6) under the object position, bit 0x01 solid), two i32 depth offsets for the left and right
halves, 3 bytes, a u32 count n and 9 n bytes (entrances), a byte, three u16, then for each of the 4 states 4 sprites,
each a u32 length and its bytes (0 = none): left half, right half, ground, shadow. The drawing adds each half's
depth offset to its sort y. No house code writes the passability: the bits stored in the map are the houses'.

**Bridges** (`most`, loader `0x10052040`): the 512-byte palette, a u32, then a **22x22 grid** of 16 px tiles from
byte 516 to 1000. The original (`0x100516e7`) places grid tile (gx, gy) in map cell `(x >> 5) + (gx >> 1) - 5`,
quarter `gx & 1` (same for y). A tile with bit 0x01 belongs to the bridge; it is open in state `s` when bit
`0x10 << s` is set, blocked otherwise. At byte 1000 a table of 10 u32 offsets (relative to 1000): abutments, their
shadow, the deck and its shadow for each of the 4 states; a 0x0 sprite means no part.

Palette entry `0xF81F` (magenta) is the transparency key in object sprites. Code: `crates/formats/src/objects.rs`,
drawing order in `crates/client/src/scene.rs`.

## Movement curves `common/CURVENEW`

81 records, one per pair of directions `next * 9 + previous` (0-7 the neighbour steps, 8 stopped): a `u8` count then
that many `(x, y, length)` `i16` triples, 2,799 bytes in all. `x`, `y` are in 1/256 px from the top-left corner of
the cell the unit is in; `length` is the distance driven so far. Straight on east `(0, 0)` is the single point
(0, 3840): the west edge of the new cell, 15 px down; stopping `(8, d)` ends in the middle (3840, 3840);
`(8, 8)` is empty. Loaded by `Game_Dll.dll` `0x1003e1d3` into `0x100b7360`. Use: `docs/movement.md`.
Code: `crates/formats/src/curves.rs`.

## Unit descriptions `desc_common/units/<name>`

Text lines `Key value...`, values separated by blanks or commas; `#hu\light.tank.common` includes another file
(path from the archive root) at that point, so later keys override it; `-` lines are comments. The game parses the
keys lowercased (`Game_Dll.dll` `0x100479c0` for vehicles: `engine`, `movespeed`, `expa_crush`, `ignoremines`,
`movedamagehp`, `movesmoke`, `canmovebackward`, `backmovespeed`...). `File` names the sprites (`sherman5` uses
`sherm-5`). Code: `crates/formats/src/desc.rs`.

## Other

- `sounds.hdr`: table of 427 (`0x1A7`) sound entries, names such as `$signal`.
- `RES/!.rsc`, `RES/*.PIC`, `*.img`: to analyse.
- `movies/*.bik`: Bink, readable with FFmpeg.
