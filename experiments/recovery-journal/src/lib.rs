//! Experimental recovery chain. CRC detects corruption, not malicious forgery.
pub mod journal;

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io {
        stage: &'static str,
        source: io::Error,
    },
    Limit(&'static str),
    Schema(u16),
    Invalid(&'static str),
    Engine(String),
    Poisoned,
    Cancelled,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub(crate) fn io(stage: &'static str, source: io::Error) -> Error {
    Error::Io { stage, source }
}
