"""Check that every .pck frame parses exactly: rows end on their length, frames end on their boundary."""
import collections, glob, os, struct, sys
sys.path.insert(0, os.path.dirname(__file__))
import pck

def check_frame(f):
    _, _, w, h, kind = struct.unpack_from('<hhHHB', f, 0)
    if kind == 0xa5:
        raise ValueError('kind 0xa5 not supported')
    pos = 9
    for y in range(h):
        (length,) = struct.unpack_from('<H', f, pos)
        line, pos = f[pos + 2:pos + 2 + length], pos + 2 + length
        x = i = 0
        while i < len(line):
            op = line[i]; i += 1
            mode, n = op >> 6, op & 0x3f
            x += n
            if kind != 0xa3:
                i += 1 if mode == 2 else (n if mode < 2 else 0)
        if x > w or i != len(line):
            raise ValueError(f'row {y} overflows')
    if pos != len(f):
        raise ValueError(f'frame ends at {pos}, size {len(f)}')

if __name__ == '__main__':
    root = sys.argv[1]
    result, failures, total = collections.Counter(), collections.Counter(), 0
    for path in sorted(glob.glob(os.path.join(root, '**', '*.[pP][cC][kK]'), recursive=True)):
        try:
            fr = pck.frames(open(path, 'rb').read())
            for f in fr:
                check_frame(f)
            result['ok'] += 1; total += len(fr)
        except Exception as e:
            kind = 'CA container' if 'CA' in str(e) else 'other'
            result[kind] += 1; failures[(os.path.relpath(path, root).split(os.sep)[0], kind)] += 1
    print(dict(result), f'{total} frames'); print(failures.most_common())
