"""Render the ground of a single-player .ssm map from mappic.rs2 rhombs.

EXPERIMENTAL: the diamond layout and palette choice are inferred, water and objects are missing.
"""
import os, struct, sys
sys.path.insert(0, os.path.dirname(__file__))
import pck, ssm_probe

WIDTHS = [4 * k + 2 for k in range(16)] + [4 * k + 2 for k in range(15, -1, -1)]   # 64x32 diamond, 1024 px

def render(map_path, climate_dir, x0, y0, cw, ch, out, palette_mode='set'):
    m = ssm_probe.parse(map_path)
    W, H = m['size']
    rhombs = open(os.path.join(climate_dir, 'mappic.rs2'), 'rb').read()
    rh_path = next(os.path.join(climate_dir, f) for f in os.listdir(climate_dir) if f.lower() == 'rhomb.pl')
    rh = open(rh_path, 'rb').read()
    pals = [[pck.rgb565(v) for v in struct.unpack_from('<256H', rh, i * 512)] for i in range(len(rh) // 512)]
    img = [[(255, 0, 255)] * (cw * 32) for _ in range(ch * 32)]
    for ty in range(y0 - 1, y0 + ch + 1):
        for tx in range(x0 - 1, x0 + cw + 2):
            if not (0 <= tx < W and 0 <= ty < H):
                continue
            v = m['tiles'][ty * W + tx]
            idx, sel = v & 0x7ff, v >> 11
            tile = rhombs[idx * 1024:(idx + 1) * 1024]
            pal = pals[sel % len(pals)] if palette_mode == 'set' else pals[0]
            cx = (tx - x0) * 32; cy = (ty - y0) * 32 + (tx & 1) * 16
            p = 0
            for y, w in enumerate(WIDTHS):
                Y = cy - 16 + y
                for x in range(w):
                    X = cx - w // 2 + x
                    if 0 <= X < cw * 32 and 0 <= Y < ch * 32 and p < len(tile):
                        img[Y][X] = pal[tile[p]]
                    p += 1
    pck.write_png(out, img)

if __name__ == '__main__':
    render(sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7]), sys.argv[7])
