"""Decode Sudden Strike .pck sprite files and .col palettes.

.col: 256 little-endian RGB565 colours.
.pck: u32 offsets[n + 1] (offsets[0] / 4 = n + 1), then n frames.
  (variant: u32 count, then offsets[count]; see frames())
  frame: i16 x0, i16 y0 (anchor offset), u16 width, u16 height, u8 kind
         kind 0xa1 / 0xa2 / 0xa9: colour sprite; 0xa3: shadow mask (0x00|n shadow, 0x80|n skip, n 7 bits);
         0xa5: unknown encoding (258 frames, not handled)
         then per row: u16 byte length, then commands until the row ends:
           0x00 | n  copy n palette pixels
           0x40 | n  copy n palette pixels blended 50 % with the background (anti-aliased edges)
           0x80 | n  repeat the next palette pixel n times
           0xC0 | n  skip n transparent pixels
Blitter: cad800.dll 0x100044f5 (row loop), 0x100045ab copy, 0x100045d0 blend, 0x10004620 run.
"""
import struct, zlib

OPAQUE, BLEND = 255, 128

def rgb565(v):
    return ((v >> 11) & 31) * 255 // 31, ((v >> 5) & 63) * 255 // 63, (v & 31) * 255 // 31

def load_palette(path):
    with open(path, 'rb') as f:
        return [rgb565(v) for v in struct.unpack('<256H', f.read(512))]

def _offset_table(data, start):
    """Read a u32 offset table at `start` whose first entry points just past the table."""
    first = struct.unpack_from('<I', data, start)[0]
    if first % 4 or not start + 4 <= first <= len(data):
        return None
    offs = struct.unpack_from(f'<{(first - start) // 4}I', data, start)
    if any(b < a for a, b in zip(offs, offs[1:])) or offs[-1] > len(data):
        return None
    return offs

def frames(data):
    """Split a .pck into frames. Two layouts exist:
    - offsets[n + 1] starting at byte 0 (frame i spans offsets[i]..offsets[i + 1]);
    - u32 header, then the same offset table starting at byte 4 (PARA, SHOT, STR...).
    Files starting with `xx 00 00 CA` (backgrounds, menu animations) are another container, not handled yet.
    """
    if data[3] == 0xCA:
        raise ValueError('CA container .pck is not supported yet')
    offs = _offset_table(data, 0) or _offset_table(data, 4)
    if offs is None:
        raise ValueError('unrecognised .pck layout')
    return [data[offs[i]:offs[i + 1]] for i in range(len(offs) - 1)]

def decode(frame, palette):
    """Return (x0, y0, width, height, pixels) with pixels[y][x] = (r, g, b, alpha) or None."""
    x0, y0, w, h, kind = struct.unpack_from('<hhHHB', frame, 0)
    if kind == 0xa3:
        return decode_shadow(frame)
    if kind == 0xa5:
        raise ValueError('frame kind 0xa5 is not supported yet')
    pos, pixels = 9, [[None] * w for _ in range(h)]
    for y in range(h):
        (length,) = struct.unpack_from('<H', frame, pos)
        line, pos = frame[pos + 2:pos + 2 + length], pos + 2 + length
        x = i = 0
        while i < len(line):
            op = line[i]; i += 1
            mode, n = op >> 6, op & 0x3f
            if mode == 3:
                x += n
                continue
            if mode == 2:
                colours, i = [line[i]] * n, i + 1
            else:
                colours, i = line[i:i + n], i + n
            alpha = BLEND if mode == 1 else OPAQUE
            for c in colours:
                if x < w:
                    pixels[y][x] = palette[c] + (alpha,)
                x += 1
    return x0, y0, w, h, pixels

def decode_shadow(frame):
    """Shadow mask: same row layout, but commands carry no colour byte (cad800.dll 0x10007938):
    0x00 | n = n shadow pixels (n up to 127), 0x80 | n = n transparent pixels.
    The game darkens each pixel once: ((p & 0xf7de) >> 1) + 0x0085 in RGB565."""
    x0, y0, w, h, _ = struct.unpack_from('<hhHHB', frame, 0)
    pos, pixels = 9, [[None] * w for _ in range(h)]
    for y in range(h):
        (length,) = struct.unpack_from('<H', frame, pos)
        line, pos = frame[pos + 2:pos + 2 + length], pos + 2 + length
        x = 0
        for op in line:
            n = op & 0x7f
            if not op & 0x80:
                for k in range(x, min(x + n, w)):
                    pixels[y][k] = (0, 0, 0, 128)
            x += n
    return x0, y0, w, h, pixels

def write_png(path, rows):
    """rows: list of lists of (r, g, b) tuples."""
    h, w = len(rows), len(rows[0])
    raw = b''.join(b'\0' + bytes(v for px in r for v in px) for r in rows)
    chunk = lambda t, d: struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d))
    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))

def contact_sheet(decoded, background=(96, 112, 64), columns=8, scale=3, pad=4):
    cw = max(d[2] for d in decoded) + pad; ch = max(d[3] for d in decoded) + pad
    rows_n = (len(decoded) + columns - 1) // columns
    sheet = [[background] * (cw * columns) for _ in range(ch * rows_n)]
    for k, (_, _, w, h, pixels) in enumerate(decoded):
        ox, oy = (k % columns) * cw + pad // 2, (k // columns) * ch + pad // 2
        for y in range(h):
            for x in range(w):
                p = pixels[y][x]
                if p:
                    bg = sheet[oy + y][ox + x]
                    sheet[oy + y][ox + x] = tuple((a * p[3] + b * (255 - p[3])) // 255 for a, b in zip(p[:3], bg))
    return [[px for px in row for _ in range(scale)] for row in sheet for _ in range(scale)]

if __name__ == '__main__':
    import sys
    pck, col, out = sys.argv[1:4]
    first, count = (int(a) for a in (sys.argv[4:6] if len(sys.argv) > 5 else (0, 16)))
    pal = load_palette(col)
    fr = frames(open(pck, 'rb').read())
    write_png(out, contact_sheet([decode(f, pal) for f in fr[first:first + count]]))
    print(f'{pck}: {len(fr)} frames, rendered {first}..{first + count - 1} to {out}')
