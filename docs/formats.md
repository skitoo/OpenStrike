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
Extractor: `tools/sue_extract.py`, verified on all 24 archives (2,963 files, sizes checked).

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
- a few odd files (`SHOT.PCK`, `STR.PCK`, menu fonts).
- `.hot` (hotspots?) and `.ani` (animations): to analyse.

## Maps `.ssm` / `.ssc` / `.smm`

- `.ssm` (single-player maps): gzip (`1F 8B`). Content to analyse.
- `.ssc`: campaigns. `.smm`: multiplayer (to check).

## Other

- `sounds.hdr`: table of 427 (`0x1A7`) sound entries, names such as `$signal`.
- `RES/!.rsc`, `RES/*.PIC`, `*.img`: to analyse.
- `movies/*.bik`: Bink, readable with FFmpeg.
