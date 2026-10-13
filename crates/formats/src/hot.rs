//! `.hot` files: attachment points of a vehicle sprite, as i32 (x, y) pairs in pixels.
//!
//! For a turreted vehicle with 32 headings (`t-26.hot`, 64 points), inferred from the data:
//! - points 0..32: turret mount on the hull, per hull frame (about 13 px above the hull anchor);
//! - points 32..64: gun muzzle relative to the turret, per turret frame. They trace an ellipse
//!   that fixes the frame orientation: frame k faces (k + 4) * 11.25 degrees clockwise from east.
//!
//! Other vehicles have more points (`bron.hot`, 128): their meaning is still unknown.

use crate::{Error, Result};

pub fn parse(data: &[u8]) -> Result<Vec<(i32, i32)>> {
    if data.is_empty() || !data.len().is_multiple_of(8) {
        return Err(Error::Invalid(format!(".hot file of {} bytes", data.len())));
    }
    Ok(data
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| {
            let (x, y) = c.split_at(4);
            (i32::from_le_bytes(x.try_into().unwrap()), i32::from_le_bytes(y.try_into().unwrap()))
        })
        .collect())
}

/// Frame of a 32-frame sprite strip that faces heading `heading` (clockwise from east, 11.25 degree steps).
pub const fn frame_for_heading(heading: u8) -> usize {
    (heading as usize + 28) % 32
}
