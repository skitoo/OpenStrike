use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("truncated data: need {need} bytes at offset {at}")]
    Truncated { need: usize, at: usize },
    #[error("bad magic, expected {expected}")]
    BadMagic { expected: &'static str },
    #[error("decompression failed: {0}")]
    Inflate(#[from] std::io::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}
