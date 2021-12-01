use std::io;

use bytes::{Buf, BytesMut};
use tokio_util::codec::Decoder;

pub struct TelnetCodec {
    prompt: String,
    current_line: Vec<u8>,
}

impl TelnetCodec {
    pub fn new(prompt: &str) -> Self {
        TelnetCodec {
            prompt: prompt.to_string(),
            current_line: Vec::with_capacity(1024),
        }
    }
}

impl Decoder for TelnetCodec {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            if src.is_empty() {
                return Ok(None);
            }

            let byte = src.get_u8();
            self.current_line.push(byte);
            if byte == 10
                || self
                    .current_line
                    .as_slice()
                    .ends_with(self.prompt.as_bytes())
            {
                let line = self.current_line.to_vec();
                self.current_line.clear();

                return Ok(Some(line));
            }
        }
    }
}
