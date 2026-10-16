//! `.sue` archives (magic `FZFS`).
//!
//! Reader in the original game: `Game_Dll.dll` `0x10092ea0`.
//!
//! ```text
//! "FZFS", u32 dir_off              directory at 8 + dir_off
//! directory = 3 blocks: u32 comp_size, u32 count, u32 elem_size, zlib data
//!   1. entries, 20 bytes: name_off, base, first_chunk, size, chunk_size
//!   2. NUL-terminated names, `\` separators
//!   3. u32 chunk offsets; chunk i spans offsets[i]..offsets[i + 1]
//! file data = zlib chunks of chunk_size bytes (stored raw when already that size)
//! ```

use std::collections::HashMap;
use std::io::Read;

use flate2::read::ZlibDecoder;

use crate::reader::Reader;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy)]
struct Entry {
    first_chunk: usize,
    size: usize,
    chunk_size: usize,
}

/// A mounted archive. Entries are decompressed on demand.
pub struct Archive {
    data: Vec<u8>,
    offsets: Vec<u32>,
    entries: HashMap<String, Entry>,
}

/// Archive names are case-insensitive and use `\` in the data; normalise to lowercase with `/`.
pub fn normalize(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

fn inflate(data: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(expected);
    ZlibDecoder::new(data).read_to_end(&mut out)?;
    Ok(out)
}

fn read_block(r: &mut Reader) -> Result<(Vec<u8>, usize)> {
    let comp = r.u32()? as usize;
    let count = r.u32()? as usize;
    let elem = r.u32()? as usize;
    let data = inflate(r.bytes(comp)?, count * elem)?;
    if data.len() != count * elem {
        return Err(Error::Invalid(format!("directory block is {} bytes, expected {}", data.len(), count * elem)));
    }
    Ok((data, count))
}

impl Archive {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        let mut r = Reader::new(&data);
        if r.bytes(4)? != b"FZFS" {
            return Err(Error::BadMagic { expected: "FZFS" });
        }
        let dir_off = r.u32()? as usize;
        let mut r = Reader::at(&data, 8 + dir_off)?;
        let (entries, count) = read_block(&mut r)?;
        let (names, _) = read_block(&mut r)?;
        let (offsets_raw, n_offsets) = read_block(&mut r)?;
        let offsets = Reader::new(&offsets_raw).u32_vec(n_offsets)?;

        let mut table = HashMap::with_capacity(count);
        let mut er = Reader::new(&entries);
        for _ in 0..count {
            let name_off = er.u32()? as usize;
            let _base = er.u32()?;
            let entry =
                Entry { first_chunk: er.u32()? as usize, size: er.u32()? as usize, chunk_size: er.u32()? as usize };
            let name = names
                .get(name_off..)
                .and_then(|s| s.split(|&b| b == 0).next())
                .ok_or_else(|| Error::Invalid(format!("name offset {name_off} out of range")))?;
            let name: String = name.iter().map(|&b| b as char).collect();
            table.insert(normalize(&name), entry);
        }
        Ok(Self { data, offsets, entries: table })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&normalize(name))
    }

    /// Decompress one entry. `Ok(None)` when the archive does not contain it.
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(e) = self.entries.get(&normalize(name)) else { return Ok(None) };
        if e.chunk_size == 0 {
            return Err(Error::Invalid("zero chunk size".into()));
        }
        let mut out = Vec::with_capacity(e.size);
        let mut chunk = e.first_chunk;
        while out.len() < e.size {
            let (start, end) = match (self.offsets.get(chunk), self.offsets.get(chunk + 1)) {
                (Some(&s), Some(&t)) if s <= t && (t as usize) <= self.data.len() => (s as usize, t as usize),
                _ => return Err(Error::Invalid(format!("chunk {chunk} out of range"))),
            };
            let raw = &self.data[start..end];
            let want = e.chunk_size.min(e.size - out.len());
            if raw.len() == want {
                out.extend_from_slice(raw);
            } else {
                out.extend(inflate(raw, want)?);
            }
            chunk += 1;
        }
        if out.len() != e.size {
            return Err(Error::Invalid(format!("entry is {} bytes, expected {}", out.len(), e.size)));
        }
        Ok(Some(out))
    }
}

/// Several archives mounted together; later archives override earlier ones (the game's `^` prefix).
#[derive(Default)]
pub struct Vfs {
    archives: Vec<Archive>,
}

impl Vfs {
    pub fn mount(&mut self, archive: Archive) {
        self.archives.push(archive);
    }

    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        for a in self.archives.iter().rev() {
            if let Some(data) = a.read(name)? {
                return Ok(Some(data));
            }
        }
        Ok(None)
    }

    pub fn require(&self, name: &str) -> Result<Vec<u8>> {
        self.read(name)?.ok_or_else(|| Error::Invalid(format!("missing resource {name}")))
    }
}
