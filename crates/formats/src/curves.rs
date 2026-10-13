//! Movement curves (`common/CURVENEW`): the points a unit drives through inside a map cell.
//!
//! Loaded by `Game_Dll.dll` `0x1003e1d3` into the table at `0x100b7360` (5-byte records: point
//! count, pointer). There are 81 records, one per pair of directions `next * 9 + previous`, where a
//! direction is one of the 8 neighbours (0 east, then clockwise, map axes) or 8 for "stopped".
//! Each record is a count followed by that many `(x, y, length)` triples of `i16`; `x` and `y` are
//! in 1/256 map pixel from the top-left corner of the cell the unit is in, `length` is the
//! distance driven so far (unused by the movement code).

use crate::reader::Reader;
use crate::{Error, Result};

pub const RECORDS: usize = 81;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i16,
    pub y: i16,
    pub length: i16,
}

/// Records indexed by `next * 9 + previous`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Curves(pub Vec<Vec<Point>>);

impl Curves {
    pub fn get(&self, next: u8, previous: u8) -> &[Point] {
        &self.0[next as usize * 9 + previous as usize]
    }
}

pub fn parse(data: &[u8]) -> Result<Curves> {
    let mut r = Reader::new(data);
    let mut records = Vec::with_capacity(RECORDS);
    for _ in 0..RECORDS {
        let n = r.u8()? as usize;
        let points = (0..n).map(|_| Ok(Point { x: r.i16()?, y: r.i16()?, length: r.i16()? })).collect::<Result<_>>()?;
        records.push(points);
    }
    if r.remaining() != 0 {
        return Err(Error::Invalid(format!("{} bytes after the curves", r.remaining())));
    }
    Ok(Curves(records))
}
