use crate::{Error, Result};

/// Little-endian cursor over a byte slice.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn at(data: &'a [u8], pos: usize) -> Result<Self> {
        if pos > data.len() {
            return Err(Error::Truncated { need: 0, at: pos });
        }
        Ok(Self { data, pos })
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        let end = end.ok_or(Error::Truncated { need: n, at: self.pos })?;
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn u32_vec(&mut self, n: usize) -> Result<Vec<u32>> {
        let raw = self.bytes(n.checked_mul(4).ok_or(Error::Invalid("table too large".into()))?)?;
        Ok(raw.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect())
    }
}

pub(crate) fn u32_at(data: &[u8], pos: usize) -> Result<u32> {
    Reader::at(data, pos)?.u32()
}
