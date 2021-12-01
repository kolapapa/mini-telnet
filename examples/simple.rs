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

    let ps_output = telnet.execute("ps aux", timeout).await?;
    println!("{}", ps_output);
    Ok(())
}
