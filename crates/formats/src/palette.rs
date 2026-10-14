//! Palettes: `.col` / `.pl` files hold 256 little-endian RGB565 colours; `rhomb.pl` holds 64 of them.
//! The colour `0xF81F` (magenta) is the transparency key: map object sprites use it in their copy runs.

use crate::{Error, Result};

pub type Rgb = [u8; 3];

pub const COLOUR_KEY: u16 = 0xF81F;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub colours: [Rgb; 256],
    /// Entries equal to [`COLOUR_KEY`].
    pub transparent: [bool; 256],
}

pub fn rgb565(v: u16) -> Rgb {
    let (r, g, b) = ((v >> 11) & 31, (v >> 5) & 63, v & 31);
    [(r * 255 / 31) as u8, (g * 255 / 63) as u8, (b * 255 / 31) as u8]
}

impl Palette {
    pub fn from_rgb565(values: &[u16; 256]) -> Self {
        Self { colours: values.map(rgb565), transparent: values.map(|v| v == COLOUR_KEY) }
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        let raw = data.get(..512).ok_or(Error::Truncated { need: 512, at: 0 })?;
        let mut values = [0u16; 256];
        for (v, pair) in values.iter_mut().zip(raw.as_chunks::<2>().0) {
            *v = u16::from_le_bytes(*pair);
        }
        Ok(Self::from_rgb565(&values))
    }

    /// Consecutive palettes, as in `rhomb.pl`.
    pub fn parse_many(data: &[u8]) -> Result<Vec<Self>> {
        if data.is_empty() || !data.len().is_multiple_of(512) {
            return Err(Error::Invalid(format!("palette file of {} bytes", data.len())));
        }
        data.as_chunks::<512>().0.iter().map(|c| Self::parse(c)).collect()
    }
}

/// The 32-bit palettes of the effect animations (`animats/*.col`, 1024 bytes), read by the `0xa9`
/// blitter (`cad800.dll` `0x10009228`): the low 16 bits are an RGB565 colour already weighted by
/// its opacity, the high byte the weight left to what is under it (`k = byte >> 3` in 32ths).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlphaPalette {
    pub entries: [u32; 256],
}

impl AlphaPalette {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let raw = data.get(..1024).ok_or(Error::Truncated { need: 1024, at: 0 })?;
        let mut entries = [0u32; 256];
        for (v, quad) in entries.iter_mut().zip(raw.as_chunks::<4>().0) {
            *v = u32::from_le_bytes(*quad);
        }
        Ok(Self { entries })
    }

    /// Weight of the background in 32ths (`entry >> 19 & 31`); 31 leaves the pixel untouched.
    pub fn background_weight(&self, index: u8) -> u8 {
        (self.entries[index as usize] >> 19 & 31) as u8
    }

    /// The weighted colour added to the background.
    pub fn colour(&self, index: u8) -> u16 {
        self.entries[index as usize] as u16
    }
}

/// What the `0xa9` blitter writes over `background` for palette entry `index`, both in RGB565:
/// each field of the background times `k / 32` (fields split by the mask `G << 16 | R | B`,
/// `0x07e0f81f`, multiplied, shifted right 5 and masked again), plus the entry's colour.
/// `None` when `k` is 31: the pixel is left alone.
pub fn blend_alpha(palette: &AlphaPalette, index: u8, background: u16) -> Option<u16> {
    const MASK: u32 = 0x07e0_f81f;
    let k = palette.background_weight(index) as u32;
    if k == 31 {
        return None;
    }
    let split = ((background as u32) << 16 | background as u32) & MASK;
    let scaled = (split.wrapping_mul(k) >> 5) & MASK;
    Some(((scaled >> 16 | scaled) as u16).wrapping_add(palette.colour(index)))
}

