"""Probe Sudden Strike .ssm maps (gzip, 'SSSM'). Layout from the QED.EXE map writer at 0x415fa0."""
import gzip, struct, sys

class Reader:
    def __init__(self, data): self.d, self.p = data, 0
    def take(self, n):
        b = self.d[self.p:self.p + n]
        if len(b) != n: raise EOFError(f'need {n} bytes at {self.p}')
        self.p += n; return b
    def u32(self): return struct.unpack('<I', self.take(4))[0]
    def i32(self): return struct.unpack('<i', self.take(4))[0]

def parse(path):
    r = Reader(gzip.open(path).read())
    m = {}
    assert r.take(4) == b'SSSM'
    m['version'] = r.u32(); m['guid'] = r.take(16).hex()
    m['header'] = [r.u32() for _ in range(5)]
    w, h = r.u32(), r.u32(); m['size'] = (w, h)
    n = r.u32(); m['briefing'] = r.take(n).rstrip(b'\0').decode('latin-1')   # u32 length including the NUL
    pw, ph = (w, h) if w <= 128 and h <= 128 else (w // 2, h // 2)        # preview, RGB565
    m['preview'] = (pw, ph, r.take(pw * ph * 2))
    m['tiles'] = struct.unpack(f'<{w * h}H', r.take(w * h * 2))           # u16: tileset << 11 | index
    m['layer2'] = struct.unpack(f'<{w * h}I', r.take(w * h * 4))
    k = r.u32(); m['tilesets'] = [r.take(16) for _ in range(k)]            # 11 x 16 bytes
    n = r.u32(); m['objects'] = [struct.unpack('<hhBBBB', r.take(8)) for _ in range(n)]
    m['end_terrain'] = r.p
    m['_rest'] = len(r.d) - r.p
    return m

if __name__ == '__main__':
    for p in sys.argv[1:]:
        try:
            m = parse(p)
            print(p.split('/')[-1], 'v', hex(m['version']), m['header'], m['size'], 'tilesets', len(m['tilesets']),
                  'objects', len(m['objects']), 'rest', m['_rest'])
        except Exception as e:
            print(p.split('/')[-1], 'ERROR', e)
