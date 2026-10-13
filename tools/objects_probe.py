"""Helpers to locate and decode map object sprites (used by iso_probe.py)."""
import os, struct, sys, zlib
sys.path.insert(0, os.path.dirname(__file__))
import pck

FLAT = {'trop', 'lay', 'fre', 'vor', 'pont', 'bred'}

def sprite_end(f, o):
    try:
        x0, y0, w, h, kind = struct.unpack_from('<hhHHB', f, o)
    except struct.error:
        return None
    if not (0 < w <= 1024 and 0 < h <= 1024 and kind in (0xa1, 0xa2, 0xa3, 0xa9) and abs(x0) < 1024 and abs(y0) < 1024):
        return None
    pos = o + 9
    for _ in range(h):
        if pos + 2 > len(f): return None
        (L,) = struct.unpack_from('<H', f, pos); line = f[pos + 2:pos + 2 + L]
        if len(line) != L: return None
        pos += 2 + L; x = i = 0
        while i < L:
            op = line[i]; i += 1; m, n = op >> 6, op & 63; x += n
            if kind != 0xa3: i += 1 if m == 2 else (n if m < 2 else 0)
        if i != L or x > w: return None
    return pos

def sprites(f, start=0):
    out, o = [], start
    while o < len(f) - 9:
        e = sprite_end(f, o)
        if e: out.append((o, f[o + 8])); o = e
        else: o += 1
    return out

def intact(f):
    s = sprites(f)
    for k, (_, kind) in enumerate(s):
        if kind == 0xa3: return s[:k + 1]
    return s[:1]

def load_set(d, files, name):
    rs = files.get(name + '.rs2') or (files.get('houses.rs2') if name == 'dom' else None)
    if not rs: return None
    data = open(os.path.join(d, rs), 'rb').read()
    c = struct.unpack_from('<I', data, 0)[0]; offs = struct.unpack_from(f'<{c + 1}I', data, 4)
    frames = [data[offs[i]:offs[i + 1]] for i in range(c)]
    pl = files.get(name + '.pl')
    pal_raw = open(os.path.join(d, pl), 'rb').read() if pl else None
    return frames, pal_raw

def palette(raw):
    vals = struct.unpack_from('<256H', raw, 0)
    return [None if v == 0xF81F else pck.rgb565(v) for v in vals]

def blit(img, spr, bx, by, W, H):
    x0, y0, w, h, px = spr
    for y in range(h):
        Y = by + y0 + y
        if not 0 <= Y < H: continue
        row = img[Y]
        for x in range(w):
            p = px[y][x]; X = bx + x0 + x
            if p and p[0] is not None and 0 <= X < W:
                row[X] = tuple((a * p[3] + c * (255 - p[3])) // 255 for a, c in zip(p[:3], row[X]))

def decode(f, o, pal):
    # palette entries may be None (colour key): decode with a stand-in then mask
    stand_in = [c if c else (1, 2, 3) for c in pal]
    x0, y0, w, h, px = pck.decode(f[o:], stand_in)
    px = [[None if (p and p[:3] == (1, 2, 3)) else p for p in row] for row in px]
    return x0, y0, w, h, px

def read_png(p):
    b = open(p, 'rb').read(); pos = 8; idat = b''; w = h = 0
    while pos < len(b):
        L = struct.unpack('>I', b[pos:pos + 4])[0]; t = b[pos + 4:pos + 8]; c = b[pos + 8:pos + 8 + L]
        if t == b'IHDR': w, h = struct.unpack('>II', c[:8])
        if t == b'IDAT': idat += c
        pos += 12 + L
    raw = zlib.decompress(idat); s = w * 3 + 1
    return [[tuple(raw[y * s + 1 + x * 3:y * s + 4 + x * 3]) for x in range(w)] for y in range(h)]
