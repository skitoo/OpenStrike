//! Sprites: `.pck` (units, effects) and `.rs2` (map objects) frame containers.
//!
//! Blitters in the original game: `cad800.dll` `0x100044f5` (colour), `0x10007938` and
//! `0x100098db` (shadows).
//!
//! ```text
//! frame = i16 x0, i16 y0 (anchor), u16 width, u16 height, u8 kind
//!         rows[height]: u16 byte length, then commands
//!   colour frames (2-bit mode, 6-bit count n)   shadow frames, kind 0xa3 (1-bit mode, 7-bit count n)
//!   0x00 | n  copy n palette pixels               0x00 | n  n shadow pixels (n up to 127)
//!   0x40 | n  copy n pixels, 50 % blend           0x80 | n  skip n pixels
//!   0x80 | n  repeat next pixel n times
//!   0xC0 | n  skip n pixels
//! ```
//!
//! A shadow pixel darkens what is under it once, however many shadows cover it: the game keeps a
//! flag per screen pixel and draws `((pixel & 0xf7de) >> 1) + 0x0085` in RGB565 (half brightness,
//! a touch of green and blue), see [`shade`].

use crate::palette::Palette;
use crate::reader::{Reader, u32_at};
use crate::{Error, Result};

pub const KIND_SHADOW: u8 = 0xa3;
pub const KIND_UNKNOWN_A5: u8 = 0xa5;

/// Alpha values used for the decoded pixels.
pub const ALPHA_OPAQUE: u8 = 255;
pub const ALPHA_BLEND: u8 = 128;
/// Shadow frames decode to a mask: white, opaque where the shadow is.
pub const SHADOW_MASK: [u8; 4] = [255, 255, 255, 255];

/// A colour seen through a shadow (`cad800.dll` `0x10007b32`), on RGB565.
pub fn shade(rgb565: u16) -> u16 {
    ((rgb565 & 0xf7de) >> 1) + 0x0085
}

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
/// A header that is exactly the frame count (the table that follows holds count + 1 offsets) is
/// checked first: when the count is a multiple of 4, the file would otherwise also look like a
/// table starting at byte 0 (`SHOT.PCK`, 192 frames).
pub fn pck_frames(data: &[u8]) -> Result<Vec<&[u8]>> {
    if data.get(3) == Some(&0xCA) {
        return Err(Error::Unsupported("CA container .pck".into()));
    }
    let counted = match (u32_at(data, 0), u32_at(data, 4)) {
        (Ok(count), Ok(first)) => (count as u64 + 1) * 4 + 4 == first as u64,
        _ => false,
    };
    let offsets = if counted { offset_table(data, 4) } else { offset_table(data, 0).or_else(|| offset_table(data, 4)) }
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

/// Length of the frame starting at `at` if one parses there exactly (every row ends on its byte
/// length and stays within the frame width), else `None`. Used to find the sprites packed inside
/// map object frames, whose headers are not all understood yet.
pub fn frame_len(data: &[u8], at: usize) -> Option<usize> {
    let mut r = Reader::at(data, at).ok()?;
    let (x0, y0) = (r.i16().ok()?, r.i16().ok()?);
    let (w, h, kind) = (r.u16().ok()? as usize, r.u16().ok()? as usize, r.u8().ok()?);
    let plausible = (1..=1024).contains(&w) && (1..=1024).contains(&h) && x0.abs() < 1024 && y0.abs() < 1024;
    if !plausible || !matches!(kind, 0xa1 | 0xa2 | KIND_SHADOW | 0xa9) {
        return None;
    }
    for _ in 0..h {
        let length = r.u16().ok()? as usize;
        let line = r.bytes(length).ok()?;
        let (mut x, mut i) = (0usize, 0usize);
        while i < line.len() {
            let op = line[i];
            i += 1;
            let n = if kind == KIND_SHADOW { op & 0x7f } else { op & 0x3f } as usize;
            x += n;
            if kind != KIND_SHADOW {
                i += match op >> 6 {
                    2 => 1,
                    3 => 0,
                    _ => n,
                };
            }
        }
        if i != line.len() || x > w {
            return None;
        }
    }
    Some(r.pos() - at)
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
            if shadow {
                let n = (op & 0x7f) as usize;
                if op & 0x80 == 0 {
                    (x..x + n).for_each(|i| put(i, [255, 255, 255], SHADOW_MASK[3]));
                }
                x += n;
                continue;
            }
            let n = (op & 0x3f) as usize;
            match op >> 6 {
                3 => {}
                2 => {
                    let Ok(c) = line.u8() else { break };
                    let c = c as usize;
                    if !palette.transparent[c] {
                        (x..x + n).for_each(|i| put(i, palette.colours[c], ALPHA_OPAQUE));
                    }
                }
                m => {
                    // a few frames count past the end of their row: the original blitter stops at
                    // the row end, so take what is there
                    let a = if m == 0 { ALPHA_OPAQUE } else { ALPHA_BLEND };
                    let take = n.min(length - line.pos());
                    for (i, &c) in line.bytes(take)?.iter().enumerate() {
                        if !palette.transparent[c as usize] {
                            put(x + i, palette.colours[c as usize], a);
                        }
                    }
                }
            }
            x += n;
        }
    }
    Ok(Image { anchor: (x0, y0), width: w as u32, height: h as u32, rgba })
}
