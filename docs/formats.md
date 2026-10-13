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
        rows[height]: u16 byte length, then commands (top 2 bits = mode, low 6 bits = n)
```

| Mode | Colour frames (observed in the blitter) | Shadow frames `0xa3` (inferred) |
|---|---|---|
| `0x00 \| n` | copy n palette pixels (`0x100045ab`) | n shadow pixels |
| `0x40 \| n` | copy n pixels blended 50 % with the background: `(src + dst - ((src ^ dst) & mask)) >> 1` (`0x100045d0`), anti-aliased edges | n lighter shadow pixels (to confirm) |
| `0x80 \| n` | repeat the next palette pixel n times (`rep stosw`, `0x10004620`) | n transparent pixels |
| `0xC0 \| n` | skip n transparent pixels | n transparent pixels |

Unit files hold the colour frames first, then the shadows (BRON: 288 frames, shadows from 160, 64 of them).
The exact shadow strength still has to be read from the shadow blitter.

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
| 32-159 | hull variants (damage states? to check) |
| 160-191 | hull shadows (first `0xa3` run) |
| 192-223 | turret, 32 headings |
| 224-255 | turret variants (to check) |
| 256-287 | turret shadows (second `0xa3` run) |

`.hot` = i32 (x, y) pairs. For the T-26: points 0-31 are the turret mount per hull frame (about (0, -13)),
points 32-63 the gun muzzle per turret frame. The muzzle points trace an ellipse that fixes the orientation:
**frame k faces (k + 4) x 11.25 degrees clockwise from east** (frame 28 east, 4 south, 12 west, 20 north).
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
  are 2, 6, ..., 62, 62, ..., 2 pixels wide.
- `rhomb.pl`: 64 palettes of 256 RGB565 colours. They are **brightness levels** (0 dark, 63 bright); the 5-bit
  selector of each cell picks one (16 almost everywhere on the sample map, 31 on a few cells).
- **Isometric projection** (inferred, see below): map pixel (x, y) is drawn at screen (x - y, (x + y) / 2), so a
  32x32 cell becomes a 64x32 rhomb, and cell (cx, cy) is centred at ((cx - cy) * 32, (cx + cy) * 16 + 16).
  Evidence: rail pieces (`trop` frame 48, a diagonal 140x68 sprite) are placed at a constant x every 80 map pixels;
  they only join into a track under this projection. The map is a diamond on screen.
- Code: `crates/formats/src/iso.rs`, `crates/formats/src/rhomb.rs`.

### Second layer and ground types

`layer2` (u32 per cell): bits 8-11 are the ground type, in the order of `desc_common/GROUNDTYPES`:
grass, lightgrass, pashnya (ploughed), swamp, darkground, lightground, **water** (6), sand.
The low byte is 0xff on water and 0x00 on land; bit 15 and bits 16+ are not understood yet.

### Water

Water cells use rhombs whose pixels sit in palette indices 146-156, which are dark green in every `rhomb.pl`
palette: the in-game water is most likely that dark green (the minimap draws it blue).
Waves are animated overlays: `common01/misc/WAVES` (`wave water 8 wolny fl_lay ...`) and
`animat/WOLNY` (`woln1`, `woln2`, `woln3`, animations in `animats/woln*.ani` + `.col`). Not rendered yet.

### Objects

Object sets are `<name>.rs2` + `<name>.pl` in the climate archive; `dom` is `houses.rs2`.
`.rs2` = u32 count, u32 offsets[count + 1], then one frame per object. Each frame is a set-specific header followed
by a run of sprites in the `.pck` frame format:

| Set | Header (as far as known) | Sprites |
|---|---|---|
| `trop`, `fre`, `vor`, `pont` | none | one flat sprite (roads, rails, paths) |
| `lay` | u8, u32 sprite length | one or two flat sprites |
| `zabor` (fences) | u32 sprite length (loader `0x10056dc0`) | colour, shadow |
| `tree` | 5 bytes, then the 6-slot table (loader `0x10055890`) | standing, shadow, fallen |
| `stand` | u32 palette index (0-4: `stand.pl` holds 5 palettes), ..., the 6-slot table at byte 205 | per slot |
| `cliff` | 8 bytes | one sprite |
| `dom` (houses) | own 512-byte palette, u16, u32, 14x14 footprint grid | per state: halves, ground, shadow |
| `most` (bridges, piers) | own 512-byte palette, two u16, 22x22 grid, sprite offset table | abutments, shadow, deck, shadow, damaged decks |

The **6-slot table** (u32 offsets in the frame, 0 = none) is: colour intact, colour destroyed, shadow intact,
shadow destroyed, ground intact, ground destroyed. A tree's "ground destroyed" is the fallen tree; a sunken barge is
an empty colour sprite, a shadow and a ground sprite. The intact state is slots 0, 2 and 4.

**House footprints** (`dom`): after the 512-byte palette come a u16, a u32, then a **14x14 grid** (read by the loader
at `Game_Dll.dll` `0x10050980`, which then reads 8 more bytes and single bytes). Each grid tile is 16x16 map pixels.
Tiles with **bit 0x01** block: the original writes houses into the map passability in `Game_Dll.dll` `0x10054cda`,
`0x10054f02` and `0x10055694`, which loop over the 14x14 grid, test `grid & 0x01`, and compute the tile as
`(x >> 4) + gx - 6`: grid tile (6, 6) is under the object position, at 16 px resolution.

**Bridges** (`most`): after the 512-byte palette come two u16, then a **22x22 grid** of 16 px tiles from byte 516 to 1000,
then a table of sprite offsets relative to byte 1000. Starting the grid at 516 is the only offset that leaves no deck
tile in the last column (otherwise rows wrap). Deck tiles have bits 0x30 set (0xff, 0x7f, 0x3f, 0xfc, 0xfe); 0x0b / 0x0f
look like railings and 0x08 / 0x09 like abutments. Grid tile (4, 4) is under the object position, fitted on the 723
summer bridges (deck over water 44 %, abutments on land 82 %). The sprites are the abutments + shadow, then the deck +
shadow, then damaged decks: the intact bridge is the first two pairs. Some `most` objects are piers rather than bridges.

Palette entry `0xF81F` (magenta) is the transparency key in object sprites.
Sets without a table: the engine locates the sprites with `sprite::frame_len` (a frame that parses exactly) and
draws every sprite up to and including the first shadow. Layers: flat sets and `0xa2` ground sprites, then shadows, then
upright sprites sorted by screen y together with the units. Code: `crates/formats/src/objects.rs`.

## Other

- `sounds.hdr`: table of 427 (`0x1A7`) sound entries, names such as `$signal`.
- `RES/!.rsc`, `RES/*.PIC`, `*.img`: to analyse.
- `movies/*.bik`: Bink, readable with FFmpeg.
