//! Palettes: `.col` / `.pl` files hold 256 little-endian RGB565 colours; `rhomb.pl` holds 64 of them.

use crate::{Error, Result};

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette(pub [Rgb; 256]);

pub fn rgb565(v: u16) -> Rgb {
    let (r, g, b) = ((v >> 11) & 31, (v >> 5) & 63, v & 31);
    [(r * 255 / 31) as u8, (g * 255 / 63) as u8, (b * 255 / 31) as u8]
}

impl Palette {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let raw = data.get(..512).ok_or(Error::Truncated { need: 512, at: 0 })?;
        let mut colours = [[0u8; 3]; 256];
        for (c, pair) in colours.iter_mut().zip(raw.as_chunks::<2>().0) {
            *c = rgb565(u16::from_le_bytes(*pair));
        }
        Ok(Self(colours))
    }

    /// Consecutive palettes, as in `rhomb.pl`.
    pub fn parse_many(data: &[u8]) -> Result<Vec<Self>> {
        if data.is_empty() || !data.len().is_multiple_of(512) {
            return Err(Error::Invalid(format!("palette file of {} bytes", data.len())));
        }
        data.as_chunks::<512>().0.iter().map(|c| Self::parse(c)).collect()
    }
}
