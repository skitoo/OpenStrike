//! Sprites: `.pck` (units, effects) and `.rs2` (map objects) frame containers.
//!
//! The drawing dispatcher of the original game (`cad800.dll` `0x10008edd`) picks the row format
//! from the frame kind: 0xa2 `0x100095a6` (also `0x100044f5`), 0xa1 `0x10008f4c`, 0xa9
//! `0x10009228`, shadows 0xa3 `0x100098db` (also `0x10007938`).
//!
//! ```text
//! frame = i16 x0, i16 y0 (anchor), u16 width, u16 height, u8 kind
//!         rows[height]: u16 byte length, then commands
//!   kind 0xa2 (2-bit mode, 6-bit count n)   kinds 0xa1, 0xa9 (7-bit count n)    kind 0xa3 shadows
//!   0x00 | n  copy n palette pixels         0x00 | n  copy n palette pixels     0x00 | n  n shadow pixels
//!   0x40 | n  copy n pixels, 50 % blend     0x80 | n, c  c = 0: skip n,         0x80 | n  skip n pixels
//!   0x80 | n  repeat next pixel n times                 else repeat c n times
//!   0xC0 | n  skip n pixels
//! ```
//!
//! On every sprite of the game's data, each kind's rows end exactly on their byte length with this
//! table (0xa1: the map objects' upright sprites; units are all 0xa2).
//!
//! A shadow pixel darkens what is under it once, however many shadows cover it: the game keeps a
//! flag per screen pixel and draws `((pixel & 0xf7de) >> 1) + 0x0085` in RGB565 (half brightness,
//! a touch of green and blue), see [`shade`].

use crate::palette::{AlphaPalette, Palette, rgb565};
use crate::reader::{Reader, u32_at};
use crate::{Error, Result};

pub const KIND_SHADOW: u8 = 0xa3;
/// Colour frames with 7-bit runs (`cad800.dll` `0x10008f4c`).
pub const KIND_WIDE: u8 = 0xa1;
/// Like [`KIND_WIDE`] with a 32-bit palette carrying a per-colour opacity (`0x10009228`), see
/// [`decode_alpha`]. Used by the effects only (`animats/*.ani`, `SHOT.PCK`).
pub const KIND_ALPHA: u8 = 0xa9;
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

/// Frame `index` of a collection, as `intrpic.dat` frames 11-13 hold them (the game's
/// `Game_Dll.dll` `0x100921c0`): 256 u32 offsets from the collection's start, one per index (wider
/// tables for indices of 0x100 and up are not read). The slice runs to the end of the collection.
pub fn collection_frame(data: &[u8], index: u8) -> Result<&[u8]> {
    let at = u32_at(data, 4 * index as usize)? as usize;
    data.get(at..).filter(|f| !f.is_empty()).ok_or_else(|| Error::Invalid(format!("collection frame {index} at {at}")))
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
            let wide = matches!(kind, KIND_SHADOW | KIND_WIDE | KIND_ALPHA);
            let n = if wide { op & 0x7f } else { op & 0x3f } as usize;
            x += n;
            if kind == KIND_SHADOW {
                continue;
            }
            i += if wide {
                if op & 0x80 != 0 { 1 } else { n }
            } else {
                match op >> 6 {
                    2 => 1,
                    3 => 0,
                    _ => n,
                }
            };
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
    if kind == KIND_ALPHA {
        return Err(Error::Unsupported("sprite frame kind 0xa9 needs a 32-bit palette (decode_alpha)".into()));
    }
    let shadow = kind == KIND_SHADOW;
    let wide = kind == KIND_WIDE;
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
            if wide {
                // 0xa1 / 0xa9 (cad800.dll 0x10009149): 7-bit counts; with bit 7, a colour byte
                // follows, 0 meaning skip, else a run of that colour
                let n = (op & 0x7f) as usize;
                if op & 0x80 != 0 {
                    let Ok(c) = line.u8() else { break };
                    let c = c as usize;
                    if c != 0 && !palette.transparent[c] {
                        (x..x + n).for_each(|i| put(i, palette.colours[c], ALPHA_OPAQUE));
                    }
                } else {
                    let take = n.min(length - line.pos());
                    for (i, &c) in line.bytes(take)?.iter().enumerate() {
                        if !palette.transparent[c as usize] {
                            put(x + i, palette.colours[c as usize], ALPHA_OPAQUE);
                        }
                    }
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

/// Decodes a `0xa9` frame (`cad800.dll` `0x10009228`) to **premultiplied** RGBA8: drawn with
/// `result = colour + background * (1 - alpha)` it gives the game's
/// `background * k / 32 + colour` (see [`crate::palette::blend_alpha`], exact per RGB565 field up
/// to rounding). Rows use the 7-bit commands of 0xa1; entries with `k = 31` stay transparent.
pub fn decode_alpha(frame: &[u8], palette: &AlphaPalette) -> Result<Image> {
    let mut r = Reader::new(frame);
    let (x0, y0) = (r.i16()? as i32, r.i16()? as i32);
    let (w, h) = (r.u16()? as usize, r.u16()? as usize);
    let kind = r.u8()?;
    if kind != KIND_ALPHA {
        return Err(Error::Invalid(format!("sprite frame kind {kind:#x}, not 0xa9")));
    }
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let length = r.u16()? as usize;
        let mut line = Reader::new(r.bytes(length)?);
        let mut x = 0usize;
        let mut put = |x: usize, c: u8| {
            let k = palette.background_weight(c);
            if x < w && k != 31 {
                let [red, green, blue] = rgb565(palette.colour(c));
                let alpha = (255 * (32 - k as u32) / 32) as u8;
                rgba[(y * w + x) * 4..][..4].copy_from_slice(&[red, green, blue, alpha]);
            }
        };
        while line.pos() < length {
            let op = line.u8()?;
            let n = (op & 0x7f) as usize;
            if op & 0x80 != 0 {
                let Ok(c) = line.u8() else { break };
                if c != 0 {
                    (x..x + n).for_each(|i| put(i, c));
                }
            } else {
                let take = n.min(length - line.pos());
                for (i, &c) in line.bytes(take)?.iter().enumerate() {
                    put(x + i, c);
                }
            }
            x += n;
        }
    }
    Ok(Image { anchor: (x0, y0), width: w as u32, height: h as u32, rgba })
}

