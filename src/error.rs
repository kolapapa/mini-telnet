use std::{io, string};

use thiserror::Error;
use tokio::time::error::Elapsed;

#[derive(Error, Debug)]
pub enum TelnetError {
    #[error("Request timeout.")]
    Timeout(#[from] Elapsed),
    #[error("io error.")]
    IOError(#[from] io::Error),
    #[error("Parse string error.")]
    ParseError(#[from] string::FromUtf8Error),
    #[error("Unknown IAC command `{0}`.")]
    UnknownIAC(String),
}
