use std::time::Duration;

use mini_telnet::Telnet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut telnet = Telnet::builder()
        .prompts(&["ubuntu@ubuntu:~$ "])
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
