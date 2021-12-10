use std::{io, string};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TelnetError {
    #[error("`{0}` Operation timeout.")]
    Timeout(String),
    #[error("io error.")]
    IOError(#[from] io::Error),
    #[error("Parse string error.")]
    ParseError(#[from] string::FromUtf8Error),
    #[error("Unknown IAC command `{0}`.")]
    UnknownIAC(String),
    #[error("Authentication failed.")]
    AuthenticationFailed,
    #[error("No more data.")]
    NoMoreData,
}
