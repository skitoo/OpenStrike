use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use openstrike_formats::Error;
use openstrike_formats::sue::{Archive, Vfs};

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn block(out: &mut Vec<u8>, data: &[u8], count: u32, elem: u32) {
    let z = zlib(data);
    out.extend((z.len() as u32).to_le_bytes());
    out.extend(count.to_le_bytes());
    out.extend(elem.to_le_bytes());
    out.extend(z);
}

/// Build an archive holding `files`, each split into zlib chunks of `chunk` bytes.
fn build(files: &[(&str, &[u8])], chunk: usize) -> Vec<u8> {
    let mut data = Vec::new();
    let mut offsets = vec![];
    let (mut entries, mut names) = (Vec::new(), Vec::new());
    for (name, content) in files {
        let first = offsets.len();
        for part in content.chunks(chunk) {
            offsets.push(8 + data.len() as u32);
            data.extend(zlib(part));
        }
        offsets.push(8 + data.len() as u32);
        for v in [names.len() as u32, 8, first as u32, content.len() as u32, chunk as u32] {
            entries.extend(v.to_le_bytes());
        }
        names.extend(name.as_bytes());
        names.push(0);
    }
    let mut out = b"FZFS".to_vec();
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(&data);
    block(&mut out, &entries, files.len() as u32, 20);
    block(&mut out, &names, names.len() as u32, 1);
    let offs: Vec<u8> = offsets.iter().flat_map(|o| o.to_le_bytes()).collect();
    block(&mut out, &offs, offsets.len() as u32, 4);
    out
}

#[test]
fn reads_multi_chunk_entries_case_insensitively() {
    let big: Vec<u8> = (0..1000u32).map(|i| (i * 7) as u8).collect();
    let a = Archive::parse(build(&[("Units\\T-26.PCK", &big), ("a.txt", b"hello")], 256)).unwrap();
    assert_eq!(a.read("units/t-26.pck").unwrap().unwrap(), big);
    assert_eq!(a.read("A.TXT").unwrap().unwrap(), b"hello");
    assert!(a.read("missing").unwrap().is_none());
}

#[test]
fn rejects_bad_magic() {
    assert!(matches!(Archive::parse(b"NOPE0000".to_vec()), Err(Error::BadMagic { .. })));
}

#[test]
fn later_mounts_override_earlier_ones() {
    let mut vfs = Vfs::default();
    vfs.mount(Archive::parse(build(&[("x", b"old")], 64)).unwrap());
    vfs.mount(Archive::parse(build(&[("x", b"new")], 64)).unwrap());
    assert_eq!(vfs.require("x").unwrap(), b"new");
}
