"""Probe the directory of a Sudden Strike .sue archive (FZFS)."""
import struct, sys, zlib

def read_block(f):
    comp, count, elem = struct.unpack('<III', f.read(12))
    data = zlib.decompress(f.read(comp))
    assert len(data) == count * elem, (len(data), count, elem)
    return data, count, elem

path = sys.argv[1]
with open(path, 'rb') as f:
    magic, dir_off = struct.unpack('<4sI', f.read(8))
    assert magic == b'FZFS'
    f.seek(8 + dir_off)
    entries, n, esz = read_block(f)
    names, _, _ = read_block(f)
    third, n3, e3 = read_block(f)
    print(f'{path}: {n} entries of {esz} bytes, names {len(names)} B, third block {n3}x{e3}, dir end {f.tell()} / size {f.seek(0, 2)}')
    for i in range(min(n, int(sys.argv[2]) if len(sys.argv) > 2 else 8)):
        name_off, a, b, c, d = struct.unpack_from('<5I', entries, i * esz)
        name = names[name_off:names.index(b'\0', name_off)].decode('latin-1')
        print(f'  {name:40s} {a:#10x} {b:#10x} {c:#10x} {d:#10x}')
    print('  third block head:', third[:64])
