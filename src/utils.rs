use bytes::{Buf, BytesMut};
use tokio_util::codec::Decoder;

use crate::error::TelnetError;

pub struct TelnetCodec {
    current_line: Vec<u8>,
}

impl Default for TelnetCodec {
    fn default() -> Self {
        TelnetCodec {
            current_line: Vec::with_capacity(1024),
        }
    }
}

#[derive(Debug)]
pub enum Item {
    Line(Vec<u8>),
    Will(u8),
    Do(u8),
}

impl Decoder for TelnetCodec {
    type Item = Item;
    type Error = TelnetError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            if src.is_empty() {
                return Ok(None);
            }
            if src[0] == 0xff {
                let (res, consume) = try_parse_iac(src.chunk());
                src.advance(consume);
                match res {
                    ParseIacResult::Invalid(err) => {
                        return Err(TelnetError::UnknownIAC(err));
                        // return Err(io::Error::new(io::ErrorKind::InvalidData, err));
                    }
                    ParseIacResult::NeedMore => return Ok(None),
                    ParseIacResult::Item(item) => return Ok(Some(item)),
                }
            } else {
                let byte = src.get_u8();
                self.current_line.push(byte);
                if byte == 10 || src.is_empty() {
                    let line = self.current_line.to_vec();
                    self.current_line.clear();

                    return Ok(Some(Item::Line(line)));
                }
            }
        }
    }
}

enum ParseIacResult {
    Invalid(String),
    NeedMore,
    Item(Item),
}

/// Returns the parsed result of the first few bytes, as well as how many bytes
/// to consume.
fn try_parse_iac(bytes: &[u8]) -> (ParseIacResult, usize) {
    if bytes.len() < 2 {
        return (ParseIacResult::NeedMore, 0);
    }
    if bytes[0] != 0xff {
        unreachable!();
    }
    if is_three_byte_iac(bytes[1]) && bytes.len() < 3 {
        return (ParseIacResult::NeedMore, 0);
    }

    match bytes[1] {
        // 250 => (ParseIacResult::Item(Item::SB), 2),
        251 => (ParseIacResult::Item(Item::Will(bytes[2])), 3),
        // 252 => (ParseIacResult::Item(Item::Wont(bytes[2])), 3),
        253 => (ParseIacResult::Item(Item::Do(bytes[2])), 3),
        // 254 => (ParseIacResult::Item(Item::Dont(bytes[2])), 3),
        cmd => (
            ParseIacResult::Invalid(format!("Unknown IAC command {}.", cmd)),
            0,
        ),
    }
}

fn is_three_byte_iac(byte: u8) -> bool {
    byte == 251 || byte == 253
}
