//! `circ64.dat` (`common.sue`): the ends of the sight rays for each radius, read by `Game_Dll.dll`
//! `0x1004da50` into `0x101fe598` (6 bytes per radius: u16 count, pointer) when `[0x10388454]` is set,
//! which a new game does (`0x10091547`). For radius 1 to 63: u16 count, then `count` records of
//! i8 dx, i8 dy, u8 angle (0-255 from west, used by sector sight, `0x1004e180`).
//! The older `circ.dat` (radius 1 to 31, 2-byte records without angle) serves old saves.

use crate::reader::Reader;
use crate::Result;

/// A ray end: offset in cells from the centre and its angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RayEnd {
    pub dx: i8,
    pub dy: i8,
    pub angle: u8,
}

/// The ray ends by radius; index 0 is empty.
pub fn parse(data: &[u8]) -> Result<Vec<Vec<RayEnd>>> {
    let mut r = Reader::new(data);
    let mut out = vec![Vec::new()];
    for _ in 1..64 {
        let n = r.u16()? as usize;
        let ends = (0..n)
            .map(|_| Ok(RayEnd { dx: r.u8()? as i8, dy: r.u8()? as i8, angle: r.u8()? }))
            .collect::<Result<_>>()?;
        out.push(ends);
    }
    Ok(out)
}
