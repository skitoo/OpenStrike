//! Unit descriptions (`desc_common/units/<name>` and the files they include).
//!
//! Text, one `Key value...` per line, values separated by blanks or commas. A line starting with
//! `#` includes another description at that point (`#hu\light.tank.common`, relative to the root of
//! the archive), so a key given after an include overrides it. Lines starting with `-` are
//! comments; `//` starts a comment anywhere. Keys are case-insensitive (the game lowercases them
//! before comparing, `Game_Dll.dll` `0x100479c0`).

use crate::sue::normalize;
use crate::{Error, Result};

const MAX_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Lowercase.
    pub key: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Desc {
    /// In file order, includes expanded in place.
    pub entries: Vec<Entry>,
}

impl Desc {
    /// Values of the last occurrence of `key`.
    pub fn get(&self, key: &str) -> Option<&[String]> {
        let key = key.to_ascii_lowercase();
        self.entries.iter().rev().find(|e| e.key == key).map(|e| &e.values[..])
    }

    /// `i`-th value of `key` as an integer (C `atoi`: leading digits, 0 if none).
    pub fn int(&self, key: &str, i: usize) -> Option<i32> {
        self.get(key)?.get(i).map(|v| atoi(v))
    }

    /// `i`-th value of `key` as a float (C `atof`, single precision as stored by the game).
    pub fn float(&self, key: &str, i: usize) -> Option<f32> {
        self.get(key)?.get(i).map(|v| atof(v))
    }
}

fn atoi(s: &str) -> i32 {
    let s = s.trim();
    let (neg, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let n = digits.bytes().take_while(u8::is_ascii_digit).fold(0i32, |n, b| n.wrapping_mul(10).wrapping_add((b - b'0') as i32));
    if neg { -n } else { n }
}

fn atof(s: &str) -> f32 {
    let s = s.trim();
    let end = s
        .char_indices()
        .take_while(|&(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+')))
        .count();
    s[..end].parse::<f64>().map_or(0.0, |v| v as f32)
}

/// Load the description `name` (an archive path such as `units/t-26`), reading files with `read`.
pub fn load(name: &str, read: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Result<Desc> {
    let mut desc = Desc::default();
    expand(&normalize(name), read, &mut desc, 0)?;
    Ok(desc)
}

fn expand(name: &str, read: &mut dyn FnMut(&str) -> Option<Vec<u8>>, desc: &mut Desc, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(Error::Invalid(format!("includes nested too deep at {name}")));
    }
    let text = read(name).ok_or_else(|| Error::Invalid(format!("description {name} not found")))?;
    for line in text.split(|&b| b == b'\n') {
        let line: String = line.iter().map(|&b| b as char).collect();
        let line = line.split("//").next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('-') {
            continue;
        }
        if let Some(include) = line.strip_prefix('#') {
            let include = include.split_whitespace().next().unwrap_or("");
            // a missing include is skipped (what the game does then is not checked yet)
            let _ = expand(&normalize(include), read, desc, depth + 1);
            continue;
        }
        let mut tokens = line.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty());
        if let Some(key) = tokens.next() {
            desc.entries.push(Entry { key: key.to_ascii_lowercase(), values: tokens.map(str::to_string).collect() });
        }
    }
    Ok(())
}
