# mini-telnet

A mini async telnet client.

## Usage

Add to Cargo.toml:

```toml
mini-telnet = "0.1.2"
```

## Example

```rust
use std::time::Duration;

use mini_telnet::Telnet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let timeout = Duration::from_secs(5);
    let mut telnet = Telnet::connect("192.168.100.2:23", timeout).await?;
    telnet
        .set_username_prompt("login: ")
        .set_password_prompt("Password: ")
        .set_prompt("ubuntu@ubuntu:~$ ")
        .login("ubuntu", "ubuntu", timeout)
        .await?;

    println!("Telnet login Success.");

    telnet.execute("echo 'test' > /tmp/temp", timeout).await?;
    let output = telnet.execute("cat /tmp/temp", timeout).await?;
    assert_eq!(output, "test\n");
    Ok(())
}
```

Part of the logic referenced from: [telnet-chat](https://github.com/Darksonn/telnet-chat)
