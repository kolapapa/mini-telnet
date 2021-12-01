use std::io;

use bytes::{Buf, BytesMut};
use futures::stream::StreamExt;
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    time::{self, Duration},
};
use tokio_util::codec::{Decoder, FramedRead};

use crate::error::TelnetError;

pub struct Telnet {
    content: Vec<u8>,
    stream: TcpStream,
    prompt: String,
    username_prompt: String,
    password_prompt: String,
}

impl Telnet {
    // Format the end of the string as a `\n`
    fn format_enter_str(s: &str) -> String {
        if !s.ends_with('\n') {
            format!("{}\n", s)
        } else {
            s.to_string()
        }
    }

    // Connect, default prompt is openwrt's prompt.
    pub async fn connect(addr: &str, timeout: Duration) -> Result<Self, TelnetError> {
        let res = time::timeout(timeout, TcpStream::connect(addr)).await?;
        Ok(Telnet {
            content: vec![],
            stream: res?,
            prompt: String::from("~# "),
            username_prompt: String::from("login: "),
            password_prompt: String::from("Password: "),
        })
    }

    pub async fn login_timeout(
        &mut self,
        username: &str,
        password: &str,
        timeout: Duration,
    ) -> Result<(), TelnetError> {
        match time::timeout(timeout, self.login(username, password)).await {
            Ok(res) => res,
            Err(e) => Err(TelnetError::Timeout(e)),
        }
    }

    async fn login(&mut self, username: &str, password: &str) -> Result<(), TelnetError> {
        let user = Telnet::format_enter_str(username);
        let pass = Telnet::format_enter_str(password);

        loop {
            self.stream.readable().await?;
            let mut buf = [0; 1024];
            match self.stream.try_read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let content = buf[0..n].to_vec();
                    if content[0] == 0xff {
                        // 设置窗口大小，不然展示会被截断（还有个字符颜色问题，应该也可以设）252 x 27
                        self.stream
                            .write(&[
                                0xff, 0xfb, 0x1f, 0xff, 0xfa, 0x1f, 0x00, 0xfc, 0x00, 0x1b, 0xff,
                                0xf0,
                            ])
                            .await?;
                    }
                    if content.ends_with(self.username_prompt.as_bytes()) {
                        self.stream.write(user.as_bytes()).await?;
                    } else if content.ends_with(self.password_prompt.as_bytes()) {
                        self.stream.write(pass.as_bytes()).await?;
                    } else if content.ends_with(self.prompt.as_bytes()) {
                        return Ok(());
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    pub async fn execute(&mut self, cmd: &str, timeout: Duration) -> Result<String, TelnetError> {
        let command = Telnet::format_enter_str(cmd);
        let (read, mut write) = self.stream.split();
        match time::timeout(timeout, write.write(command.as_bytes())).await {
            Ok(res) => res?,
            Err(e) => return Err(TelnetError::Timeout(e)),
        };
        let mut telnet = FramedRead::new(read, TelnetCodec::new(self.prompt.as_str()));

        loop {
            match time::timeout(timeout, telnet.next()).await {
                Ok(res) => {
                    if let Some(item) = res {
                        let mut line = item?;
                        if line.ends_with(self.prompt.as_bytes()) {
                            break;
                        }
                        if line.starts_with(cmd.as_bytes()) {
                            continue;
                        }
                        self.content.append(&mut line);
                    }
                }
                Err(e) => return Err(TelnetError::Timeout(e)),
            }
        }
        let content = self.content.clone();
        self.content.clear();

        let output = String::from_utf8(content)?;
        Ok(output)
    }

    pub fn set_prompt(&mut self, prompt: &str) -> &mut Self {
        self.prompt = prompt.to_string();
        self
    }

    pub fn set_username_prompt(&mut self, prompt: &str) -> &mut Self {
        self.username_prompt = prompt.to_string();
        self
    }

    pub fn set_password_prompt(&mut self, prompt: &str) -> &mut Self {
        self.password_prompt = prompt.to_string();
        self
    }
}

struct TelnetCodec {
    prompt: String,
    current_line: Vec<u8>,
}

impl TelnetCodec {
    fn new(prompt: &str) -> Self {
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
            match byte {
                10 => {
                    self.current_line.push(byte);
                    let line = self.current_line.to_vec();
                    self.current_line.clear();

                    return Ok(Some(line));
                }
                0..=31 => {}
                _ => {
                    self.current_line.push(byte);
                    if self
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
    }
}
