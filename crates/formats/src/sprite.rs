//! Sprites: `.pck` (units, effects) and `.rs2` (map objects) frame containers.
//!
//! Blitter in the original game: `cad800.dll` `0x100044f5`.
//!
//! ```text
//! frame = i16 x0, i16 y0 (anchor), u16 width, u16 height, u8 kind
//!         rows[height]: u16 byte length, then commands (2-bit mode, 6-bit count n):
//!           colour frames               shadow frames (kind 0xa3)
//!   0x00  copy n palette pixels         n shadow pixels
//!   0x40  copy n pixels, 50 % blend     n lighter shadow pixels
//!   0x80  repeat next pixel n times     n transparent pixels
//!   0xC0  skip n pixels                 n transparent pixels
//! ```

use crate::palette::Palette;
use crate::reader::{Reader, u32_at};
use crate::{Error, Result};

pub const KIND_SHADOW: u8 = 0xa3;
pub const KIND_UNKNOWN_A5: u8 = 0xa5;

/// Alpha values used for the decoded pixels.
pub const ALPHA_OPAQUE: u8 = 255;
pub const ALPHA_BLEND: u8 = 128;
pub const ALPHA_SHADOW: u8 = 128;
pub const ALPHA_SHADOW_EDGE: u8 = 64;

/// A decoded frame as straight RGBA8, with the anchor offset to add to the draw position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub anchor: (i32, i32),
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn offset_table(data: &[u8], start: usize) -> Option<Vec<usize>> {
    let first = u32_at(data, start).ok()? as usize;
    if !first.is_multiple_of(4) || first < start + 4 || first > data.len() {
        return None;
    }
    let offsets = Reader::at(data, start).ok()?.u32_vec((first - start) / 4).ok()?;
    let offsets: Vec<usize> = offsets.into_iter().map(|o| o as usize).collect();
    let sorted = offsets.windows(2).all(|w| w[0] <= w[1]);
    (sorted && *offsets.last()? <= data.len()).then_some(offsets)
}

fn slices<'a>(data: &'a [u8], offsets: &[usize]) -> Vec<&'a [u8]> {
    offsets.windows(2).map(|w| &data[w[0]..w[1]]).collect()
}

/// Split a `.pck` into frames: either `offsets[n + 1]` from byte 0, or a u32 header then the table.
pub fn pck_frames(data: &[u8]) -> Result<Vec<&[u8]>> {
    if data.get(3) == Some(&0xCA) {
        return Err(Error::Unsupported("CA container .pck".into()));
    }
    let offsets = offset_table(data, 0)
        .or_else(|| offset_table(data, 4))
        .ok_or_else(|| Error::Invalid("unrecognised .pck layout".into()))?;
    Ok(slices(data, &offsets))
}

/// Split a `.rs2` into frames: u32 count, then `offsets[count + 1]`. Empty frames are kept.
pub fn rs2_frames(data: &[u8]) -> Result<Vec<&[u8]>> {
    let mut r = Reader::new(data);
    let count = r.u32()? as usize;
    let offsets: Vec<usize> = r.u32_vec(count + 1)?.into_iter().map(|o| o as usize).collect();
    if offsets.windows(2).any(|w| w[0] > w[1]) || offsets.last().is_some_and(|&o| o > data.len()) {
        return Err(Error::Invalid("bad .rs2 offset table".into()));
    }
    Ok(slices(data, &offsets))
}

/// Frame kind byte, or `None` for an empty frame.
pub fn frame_kind(frame: &[u8]) -> Option<u8> {
    frame.get(8).copied()
}

pub fn decode(frame: &[u8], palette: &Palette) -> Result<Image> {
    let mut r = Reader::new(frame);
    let (x0, y0) = (r.i16()? as i32, r.i16()? as i32);
    let (w, h) = (r.u16()? as usize, r.u16()? as usize);
    let kind = r.u8()?;
    if kind == KIND_UNKNOWN_A5 {
        return Err(Error::Unsupported("sprite frame kind 0xa5".into()));
    }
    let shadow = kind == KIND_SHADOW;
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let length = r.u16()? as usize;
        let mut line = Reader::new(r.bytes(length)?);
        let mut x = 0usize;
        let mut put = |x: usize, rgb: [u8; 3], a: u8| {
            if x < w {
                rgba[(y * w + x) * 4..][..4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
            }
        };
        while line.pos() < length {
            let op = line.u8()?;
            let n = (op & 0x3f) as usize;
            match (op >> 6, shadow) {
                (3, _) | (2, true) => {}
                (m, true) => {
                    let a = if m == 0 { ALPHA_SHADOW } else { ALPHA_SHADOW_EDGE };
                    (x..x + n).for_each(|i| put(i, [0, 0, 0], a));
                }
                (2, false) => {
                    let c = palette.0[line.u8()? as usize];
                    (x..x + n).for_each(|i| put(i, c, ALPHA_OPAQUE));
                }
                (m, false) => {
                    let a = if m == 0 { ALPHA_OPAQUE } else { ALPHA_BLEND };
                    for (i, &c) in line.bytes(n)?.iter().enumerate() {
                        put(x + i, palette.0[c as usize], a);
                    }
                }
            }
            x += n;
        }
        if x > w {
            return Err(Error::Invalid(format!("row {y} is {x} pixels wide, frame is {w}")));
        }
    }
    Ok(Image { anchor: (x0, y0), width: w as u32, height: h as u32, rgba })
}
