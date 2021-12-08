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

    telnet.execute("echo 'test' > /tmp/temp").await?;
    let output = telnet.execute("cat /tmp/temp").await?;
    assert_eq!(output, "test\n");

    let output = telnet
        .execute(
            r#"cat <<EOF
first line
 second line
 third line
 final line
!
EOF
"#,
        )
        .await?;

    assert_eq!(
        output,
        "first line\n second line\n third line\n final line\n!\n"
    );

    Ok(())
}
