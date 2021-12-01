mod codec;
pub mod error;

use encoding::DecoderTrap;
use encoding::{all::GBK, Encoding};
use futures::stream::StreamExt;
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    time::{self, Duration},
};
use tokio_util::codec::FramedRead;

use crate::codec::{Item, TelnetCodec};
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

    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
        timeout: Duration,
    ) -> Result<(), TelnetError> {
        let user = Telnet::format_enter_str(username);
        let pass = Telnet::format_enter_str(password);

        let (read, mut write) = self.stream.split();
        let mut telnet = FramedRead::new(read, TelnetCodec::default());

        loop {
            match time::timeout(timeout, telnet.next()).await {
                Ok(res) => {
                    if let Some(res) = res {
                        match res? {
                            Item::Do(i) | Item::Dont(i) => {
                                // set window size
                                if i == 0x1f {
                                    write
                                        .write(&[
                                            0xff, 0xfb, 0x1f, 0xff, 0xfa, 0x1f, 0x00, 0xfc, 0x00,
                                            0x1b, 0xff, 0xf0,
                                        ])
                                        .await?;
                                } else {
                                    write.write(&[0xff, 0xfc, i]).await?;
                                }
                            }
                            Item::Will(i) | Item::Wont(i) => {
                                write.write(&[0xff, 0xfe, i]).await?;
                            }
                            Item::Line(content) => {
                                if content.ends_with(self.username_prompt.as_bytes()) {
                                    write.write(user.as_bytes()).await?;
                                } else if content.ends_with(self.password_prompt.as_bytes()) {
                                    write.write(pass.as_bytes()).await?;
                                } else if content.ends_with(self.prompt.as_bytes()) {
                                    return Ok(());
                                }
                            }
                            item => return Err(TelnetError::UnknownIAC(format!("{:?}", item))),
                        }
                    }
                }
                Err(e) => return Err(TelnetError::Timeout(e)),
            }
        }
    }

    pub async fn execute(&mut self, cmd: &str, timeout: Duration) -> Result<String, TelnetError> {
        let command = Telnet::format_enter_str(cmd);
        let (read, mut write) = self.stream.split();
        match time::timeout(timeout, write.write(command.as_bytes())).await {
            Ok(res) => res?,
            Err(e) => return Err(TelnetError::Timeout(e)),
        };
        let mut telnet = FramedRead::new(read, TelnetCodec::default());

        loop {
            match time::timeout(timeout, telnet.next()).await {
                Ok(res) => {
                    if let Some(item) = res {
                        if let Item::Line(mut line) = item? {
                            if line.ends_with(self.prompt.as_bytes()) {
                                break;
                            }
                            if line.starts_with(cmd.as_bytes()) {
                                continue;
                            }
                            self.content.append(&mut line);
                        }
                    }
                }
                Err(e) => return Err(TelnetError::Timeout(e)),
            }
        }
        let output = String::from_utf8(self.content.clone());
        let result = match output {
            Ok(s) => Ok(s),
            Err(e) => match GBK.decode(&self.content, DecoderTrap::Strict) {
                Ok(gbk_out) => Ok(gbk_out),
                Err(_) => Err(TelnetError::ParseError(e)),
            },
        };
        self.content.clear();
        result
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
