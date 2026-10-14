//! `.hot` files: attachment points of a vehicle sprite, as i32 (x, y) pairs in pixels.
//!
//! For a turreted vehicle with 32 headings (`t-26.hot`, 64 points), inferred from the data:
//! - points 0..32: turret mount on the hull, per hull frame (about 13 px above the hull anchor);
//! - points 32..64: gun muzzle relative to the turret, per turret frame (they trace an ellipse).
//!
//! Frame k of a strip faces map heading k (`Game_Dll.dll` `0x1000eb7b`).
//!
//! Other vehicles have more points (`bron.hot`, 128): their meaning is still unknown.

use crate::{Error, Result};

/// The points of a `.hot` file. An empty file is valid: the tank type loader (`Game_Dll.dll`
/// `0x10043090`) reads up to `0x300` bytes into a table its constructor zeroed, so missing points
/// are `(0, 0)` (the Goliath, `bla.hot`, has none).
pub fn parse(data: &[u8]) -> Result<Vec<(i32, i32)>> {
    if !data.len().is_multiple_of(8) {
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
