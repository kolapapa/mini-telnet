# mini-telnet

[![Crates.io](https://img.shields.io/crates/v/mini-telnet.svg)](https://crates.io/crates/mini-telnet)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/kolapapa/mini-telnet/blob/main/LICENSE)
[![API docs](https://docs.rs/mini-telnet/badge.svg)](http://docs.rs/mini-telnet)

A mini async telnet client.

## Usage

Add to Cargo.toml:

```toml
mini-telnet = "0.1.4"
```

## Example

```rust
use std::time::Duration;

use mini_telnet::Telnet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut telnet = Telnet::builder()
        .prompt("ubuntu@ubuntu:~$ ")
        .login_prompt("login: ", "Password: ")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(5))
        .connect("192.168.100.2:23")
        .await?;
    telnet.login("ubuntu", "ubuntu").await?;

    assert_eq!(
        telnet.normal_execute("echo 'haha'").await?,
        "echo 'haha'\nhaha\n",
    );

    assert_eq!(telnet.execute("echo 'haha'").await?, "haha\n");
    Ok(())
}
```

Part of the logic referenced from: [telnet-chat](https://github.com/Darksonn/telnet-chat)
