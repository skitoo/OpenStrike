"""Extract Sudden Strike .sue archives (FZFS).

Layout (reader at Game_Dll.dll 0x10092ea0):
  "FZFS", u32 dir_off               directory starts at 8 + dir_off
  directory = 3 blocks, each: u32 comp_size, u32 count, u32 elem_size, zlib data
    1. entries, 20 bytes: u32 name_off, u32 base, u32 first_chunk, u32 size, u32 chunk_size
    2. names, NUL-terminated strings
    3. chunk offsets, u32; chunk i of a file spans offsets[i]..offsets[i+1]
  file data = consecutive zlib chunks of chunk_size uncompressed bytes
"""
import os, struct, sys, zlib

def read_block(f):
    comp, count, elem = struct.unpack('<III', f.read(12))
    data = zlib.decompress(f.read(comp))
    assert len(data) == count * elem
    return data, count

def read_archive(path):
    with open(path, 'rb') as f:
        blob = f.read()
    magic, dir_off = struct.unpack_from('<4sI', blob, 0)
    if magic != b'FZFS':
        raise ValueError(f'{path}: not an FZFS archive')
    import io
    f = io.BytesIO(blob)
    f.seek(8 + dir_off)
    entries, n = read_block(f)
    names, _ = read_block(f)
    offs_raw, n_offs = read_block(f)
    offs = struct.unpack(f'<{n_offs}I', offs_raw)
    for i in range(n):
        name_off, base, first, size, chunk = struct.unpack_from('<5I', entries, i * 20)
        name = names[name_off:names.index(b'\0', name_off)].decode('latin-1')
        out, j = bytearray(), first
        while len(out) < size:
            raw = blob[offs[j]:offs[j + 1]]
            want = min(chunk, size - len(out))
            out += raw if len(raw) == want else zlib.decompress(raw)
            j += 1
        if len(out) != size:
            raise ValueError(f'{path}:{name}: got {len(out)} bytes, expected {size}')
        yield name, bytes(out)

if __name__ == '__main__':
    out_root = sys.argv[1]
    for path in sys.argv[2:]:
        dest = os.path.join(out_root, os.path.splitext(os.path.basename(path))[0])
        count = total = 0
        for name, data in read_archive(path):
            target = os.path.join(dest, *name.split('\\'))
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, 'wb') as out:
                out.write(data)
            count += 1; total += len(data)
        print(f'{os.path.basename(path)}: {count} files, {total / 1e6:.1f} MB')
