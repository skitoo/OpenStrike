//! Pure decoders for the Sudden Strike (Fireglow, 2000) data formats.
//!
//! Every function takes bytes and returns plain data: no I/O beyond what the caller hands in,
//! no rendering library. Format notes and the original code addresses live in `docs/formats.md`.

pub mod curves;
pub mod desc;
mod error;
pub mod hot;
pub mod ini;
pub mod interface;
pub mod iso;
pub mod map;
pub mod objects;
pub mod palette;
mod reader;
pub mod rhomb;
pub mod sprite;
pub mod sue;

pub use error::{Error, Result};
