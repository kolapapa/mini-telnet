mod codec;
pub mod error;

use encoding::DecoderTrap;
use encoding::{all::GB18030, all::GBK, Encoding};
use futures::stream::StreamExt;
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    time::{self, Duration},
};
use tokio_util::codec::FramedRead;

use crate::codec::{Item, TelnetCodec};
use crate::error::TelnetError;

#[derive(Debug, Default)]
pub struct TelnetBuilder {
    prompts: Vec<String>,
    username_prompt: String,
    password_prompt: String,
    connect_timeout: Duration,
    timeout: Duration,
}

impl TelnetBuilder {
    /// Set the telnet server prompt, as many characters as possible.(`~` or `#` is not good. May misjudge).
    pub fn prompt<T: ToString>(mut self, prompt: T) -> TelnetBuilder {
        self.prompts = vec![prompt.to_string()];
        self
    }

    /// Set the telnet server prompts, as many characters as possible.(`~` or `#` is not good. May misjudge).
    /// If `prompts` is set, `prompt` will be overwritten.
    pub fn prompts<T: ToString>(mut self, prompts: &[T]) -> TelnetBuilder {
        self.prompts = prompts.iter().map(|p| p.to_string()).collect();
        self
    }

    /// Login prompt, the common ones are `login: ` and `Password: ` or `Username:` and `Password:`.
    pub fn login_prompt(mut self, user_prompt: &str, pass_prompt: &str) -> TelnetBuilder {
        self.username_prompt = user_prompt.to_string();
        self.password_prompt = pass_prompt.to_string();
        self
    }

    /// Set the timeout for `TcpStream` connect remote addr.
    pub fn connect_timeout(mut self, connect_timeout: Duration) -> TelnetBuilder {
        self.connect_timeout = connect_timeout;
        self
    }

    /// Set the timeout for the operation.
    pub fn timeout(mut self, timeout: Duration) -> TelnetBuilder {
        self.timeout = timeout;
        self
    }

    /// Establish a connection with the remote telnetd.
    pub async fn connect(self, addr: &str) -> Result<Telnet, TelnetError> {
        match time::timeout(self.connect_timeout, TcpStream::connect(addr)).await {
            Ok(res) => Ok(Telnet {
                content: vec![],
                stream: res?,
                timeout: self.timeout,
                prompts: self.prompts,
                username_prompt: self.username_prompt,
                password_prompt: self.password_prompt,
            }),
            Err(_) => Err(TelnetError::Timeout(format!(
                "Connect remote addr({})",
                addr
            ))),
        }
    }
}

pub struct Telnet {
    timeout: Duration,
    content: Vec<String>,
    stream: TcpStream,
    prompts: Vec<String>,
    username_prompt: String,
    password_prompt: String,
}

impl Telnet {
    /// Create a `TelnetBuilder`
    pub fn builder() -> TelnetBuilder {
        TelnetBuilder::default()
    }
    // Format the end of the string as a `\n`
    fn format_enter_str(s: &str) -> String {
        if !s.ends_with('\n') {
            format!("{}\n", s)
        } else {
            s.to_string()
        }
    }

    /// Login remote telnet daemon, only retry one time.
    /// # Examples
    ///
    /// ```no_run
    /// let mut client = Telnet::builder()
    ///     .prompt("username@hostname:$ ")
    ///     .login_prompt("login: ", "Password: ")
    ///     .connect_timeout(Duration::from_secs(3))
    ///     .connect("192.168.0.1:23").await?;
    ///
    /// match client.login("username", "password").await {
    ///     Ok(_) => println!("login success."),
    ///     Err(e) => println!("login failed: {}", e),
    /// };
    /// ```
    ///
    pub async fn login(&mut self, username: &str, password: &str) -> Result<(), TelnetError> {
        let user = Telnet::format_enter_str(username);
        let pass = Telnet::format_enter_str(password);

        // Only retry one time, if password is input, then set with `true`;
        let mut auth_failed = false;

        let (read, mut write) = self.stream.split();
        let mut telnet = FramedRead::new(read, TelnetCodec::default());

        loop {
            match time::timeout(self.timeout, telnet.next()).await {
                Ok(res) => {
                    match res {
                        Some(res) => {
                            match res? {
                                Item::Do(i) | Item::Dont(i) => {
                                    // set window size
                                    if i == 0x1f {
                                        write
                                            .write_all(&[
                                                0xff, 0xfb, 0x1f, 0xff, 0xfa, 0x1f, 0x00, 0xfc,
                                                0x00, 0x1b, 0xff, 0xf0,
                                            ])
                                            .await?;
                                    } else {
                                        write.write_all(&[0xff, 0xfc, i]).await?;
                                    }
                                }
                                Item::Will(i) | Item::Wont(i) => {
                                    write.write_all(&[0xff, 0xfe, i]).await?;
                                }
                                Item::Line(content) => {
                                    if content.ends_with(self.username_prompt.as_bytes()) {
                                        if auth_failed {
                                            return Err(TelnetError::AuthenticationFailed);
                                        }
                                        write.write_all(user.as_bytes()).await?;
                                    } else if content.ends_with(self.password_prompt.as_bytes()) {
                                        write.write_all(pass.as_bytes()).await?;
                                        auth_failed = true;
                                    } else if self
                                        .prompts
                                        .iter()
                                        .filter(|p| content.ends_with(p.as_bytes()))
                                        .count()
                                        != 0
                                    {
                                        return Ok(());
                                    }
                                }
                                item => return Err(TelnetError::UnknownIAC(format!("{:?}", item))),
                            }
                        }
                        None => return Err(TelnetError::NoMoreData),
                    };
                }
                Err(_) => return Err(TelnetError::Timeout("login".to_string())),
            }
        }
    }

    /// Execute command, and filter it input message by line count.
    ///
    /// # Examples
    ///
    /// ```no_run
    ///assert_eq!(telnet.execute("echo 'haha'").await?, "haha\n");
    /// ```
    ///
    pub async fn execute(&mut self, cmd: &str) -> Result<String, TelnetError> {
        let command = Telnet::format_enter_str(cmd);
        let mut incomplete_line: Vec<u8> = vec![];
        let mut line_feed_cnt = command.lines().count() as isize;
        let mut real_output = false;

        let (read, mut write) = self.stream.split();
        match time::timeout(self.timeout, write.write(command.as_bytes())).await {
            Ok(res) => res?,
            Err(_) => return Err(TelnetError::Timeout("write cmd".to_string())),
        };
        let mut telnet = FramedRead::new(read, TelnetCodec::default());

        loop {
            match time::timeout(self.timeout, telnet.next()).await {
                Ok(res) => match res {
                    Some(item) => {
                        if let Item::Line(mut line) = item? {
                            // ignore prompt line
                            if self
                                .prompts
                                .iter()
                                .filter(|p| line.ends_with(p.as_bytes()))
                                .count()
                                != 0
                            {
                                break;
                            }
                            // ignore command line echo
                            if line.ends_with(&[10]) && line_feed_cnt > 0 {
                                line_feed_cnt -= 1;
                                if line_feed_cnt == 0 {
                                    real_output = true;
                                    continue;
                                }
                            }

                            if !real_output {
                                continue;
                            }

                            if !line.ends_with(&[10]) || !incomplete_line.is_empty() {
                                incomplete_line.append(&mut line);
                            } else {
                                self.content.push(decode(&line)?);
                                continue;
                            }
                            // ignore command line
                            if self
                                .prompts
                                .iter()
                                .filter(|p| incomplete_line.ends_with(p.as_bytes()))
                                .count()
                                != 0
                            {
                                break;
                            }
                            if incomplete_line.ends_with(&[10]) {
                                self.content.push(decode(&incomplete_line)?);
                                incomplete_line.clear();
                            }
                        }
                    }
                    None => return Err(TelnetError::NoMoreData),
                },
                Err(_) => return Err(TelnetError::Timeout("read next framed".to_string())),
            }
        }
        let result = self.content.join("\n");
        self.content.clear();
        Ok(result)
    }

    /// All echoed content is returned when the command is executed.(**Note** that this may contain some
    /// useless information, such as prompts, which need to be filtered and processed by yourself.)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// assert_eq!(
    ///     "echo 'haha'\nhaha\n",
    ///     telnet.normal_execute("echo 'haha'").await?
    /// );
    ///```
    ///
    pub async fn normal_execute(&mut self, cmd: &str) -> Result<String, TelnetError> {
        let command = Telnet::format_enter_str(cmd);
        let mut incomplete_line: Vec<u8> = vec![];

        let (read, mut write) = self.stream.split();
        match time::timeout(self.timeout, write.write(command.as_bytes())).await {
            Ok(res) => res?,
            Err(_) => return Err(TelnetError::Timeout("write cmd".to_string())),
        };
        let mut telnet = FramedRead::new(read, TelnetCodec::default());

        loop {
            match time::timeout(self.timeout, telnet.next()).await {
                Ok(res) => match res {
                    Some(item) => {
                        if let Item::Line(mut line) = item? {
                            if self
                                .prompts
                                .iter()
                                .filter(|p| line.ends_with(p.as_bytes()))
                                .count()
                                != 0
                            {
                                break;
                            }

                            if !line.ends_with(&[10]) || !incomplete_line.is_empty() {
                                incomplete_line.append(&mut line);
                            } else {
                                self.content.push(decode(&line)?);
                                continue;
                            }
                            // ignore command line
                            if self
                                .prompts
                                .iter()
                                .filter(|p| incomplete_line.ends_with(p.as_bytes()))
                                .count()
                                != 0
                            {
                                break;
                            }
                            if incomplete_line.ends_with(&[10]) {
                                self.content.push(decode(&incomplete_line)?);
                                incomplete_line.clear();
                            }
                        }
                    }
                    None => return Err(TelnetError::NoMoreData),
                },
                Err(_) => return Err(TelnetError::Timeout("read next framed".to_string())),
            }
        }
        let result = self.content.join("\n");
        self.content.clear();
        Ok(result)
    }
}

fn decode(line: &[u8]) -> Result<String, TelnetError> {
    match String::from_utf8(line.to_vec()) {
        Ok(result) => Ok(result),
        Err(e) => {
            if let Ok(result) = GBK.decode(line, DecoderTrap::Strict) {
                return Ok(result);
            }

            if let Ok(result) = GB18030.decode(line, DecoderTrap::Strict) {
                return Ok(result);
            }
            Err(TelnetError::ParseError(e))
        }
    }
}
