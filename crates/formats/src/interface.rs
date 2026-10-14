//! Interface texts and key names.
//!
//! `intrtext.dat` (`common.sue`, loaded by `Game_Dll.dll` `0x1005abd0`): u32 offsets, the first one
//! / 4 being their count, then NUL-terminated texts (Windows-1252, read here as Latin-1), where
//! `~key_xx~` stands for the name of the key bound to `xx`.
//!
//! `key.map` (the install's root): one binding per line, `key_<action>`, then the virtual key code
//! in hex and the key's name (`key_00`, tabs, `41 A`; `key_Give`, tabs, `08 BackSpace`).

use std::collections::HashMap;

use crate::reader::Reader;
use crate::{Error, Result};

/// The texts of `intrtext.dat`, by index.
pub fn texts(data: &[u8]) -> Result<Vec<String>> {
    let first = Reader::new(data).u32()? as usize;
    if first == 0 || !first.is_multiple_of(4) || first > data.len() {
        return Err(Error::Invalid(format!("intrtext.dat: first offset {first}")));
    }
    let offsets = Reader::new(data).u32_vec(first / 4)?;
    offsets
        .iter()
        .map(|&o| {
            let text = data.get(o as usize..).ok_or_else(|| Error::Invalid(format!("intrtext.dat: offset {o}")))?;
            Ok(text.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect())
        })
        .collect()
}

/// The key names of `key.map`, by action (`00`, `HM`, `g1`...: the name without `key_`).
pub fn key_names(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let action = words.next()?.strip_prefix("key_")?;
            words.next()?; // the virtual key code
            let name = words.collect::<Vec<_>>().join(" ");
            Some((action.to_string(), name))
        })
        .collect()
}
