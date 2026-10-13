"""Experimental: isometric projection test. Map pixel (x, y) -> screen (x - y, (x + y) / 2);
cell (cx, cy) -> rhomb centred at ((cx - cy) * 32, (cx + cy) * 16 + 16)."""
import os, struct, sys
sys.path.insert(0, os.path.dirname(__file__))
import pck, ssm_probe, objects_probe as op

W_ROWS = [4 * k + 2 for k in range(16)] + [4 * k + 2 for k in range(15, -1, -1)]

def main(map_path, climate_dir, sx0, sy0, sw, sh, out):
    m = ssm_probe.parse(map_path); W, H = m['size']
    files = {f.lower(): f for f in os.listdir(climate_dir)}
    rhombs = open(os.path.join(climate_dir, 'mappic.rs2'), 'rb').read()
    rh = open(os.path.join(climate_dir, files['rhomb.pl']), 'rb').read()
    pals = [[pck.rgb565(v) for v in struct.unpack_from('<256H', rh, i * 512)] for i in range(len(rh) // 512)]
    img = [[(0, 0, 0)] * sw for _ in range(sh)]
    for cy in range(H):
        for cx in range(W):
            X, Y = (cx - cy) * 32 - sx0, (cx + cy) * 16 + 16 - sy0
            if not (-40 < X < sw + 40 and -20 < Y < sh + 20): continue
            v = m['tiles'][cy * W + cx]; idx, sel = v & 0x7ff, v >> 11
            t = rhombs[idx * 1024:(idx + 1) * 1024]; pal = pals[sel % len(pals)]; p = 0
            for r, w in enumerate(W_ROWS):
                yy = Y - 16 + r
                for i in range(w):
                    xx = X - w // 2 + i
                    if 0 <= xx < sw and 0 <= yy < sh and p < len(t): img[yy][xx] = pal[t[p]]
                    p += 1
    names = [t.rstrip(b'\0').decode() for t in m['tilesets']]
    sets = {i: op.load_set(climate_dir, files, n) for i, n in enumerate(names)}
    flat, shadows, upright = [], [], []
    for (x, y, s, idx, b2, b3) in m['objects']:
        if not sets.get(s): continue
        frames, pal_raw = sets[s]
        if idx >= len(frames) or not frames[idx]: continue
        f = frames[idx]
        own = names[s] in ('dom', 'most')
        if not (pal_raw or own): continue
        pal = op.palette(f if own else pal_raw)
        bx, by = x - y - sx0, (x + y) // 2 - sy0
        if not (-300 < bx < sw + 300 and -300 < by < sh + 300): continue
        for o, kind in op.intact(f):
            item = (by, bx, f, o, pal)
            (shadows if kind == 0xa3 else flat if names[s] in op.FLAT or kind == 0xa2 else upright).append(item)
    for layer in (flat, shadows, sorted(upright, key=lambda t: t[0])):
        for by, bx, f, o, pal in layer:
            op.blit(img, op.decode(f, o, pal), bx, by, sw, sh)
    pck.write_png(out, img)

if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7]), sys.argv[7])
