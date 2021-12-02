# mini-telnet

A mini async telnet client.

## Usage

Add to Cargo.toml:

```toml
mini-telnet = "0.1.3"
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

    println!("Telnet login Success.");

    telnet.execute("echo 'test' > /tmp/temp").await?;
    let output = telnet.execute("cat /tmp/temp").await?;
    assert_eq!(output, "test\n");
    Ok(())
}

```

Part of the logic referenced from: [telnet-chat](https://github.com/Darksonn/telnet-chat)
