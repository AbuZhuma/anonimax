use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct Tor {
    pub enabled: bool,
    pub socks: String,
    pub control: String,
}

impl Tor {
    pub fn new() -> Self {
        Self {
            enabled: false,
            socks: "127.0.0.1:9050".to_string(),
            control: "127.0.0.1:9051".to_string(),
        }
    }

    pub fn isolated_proxy(&self) -> String {
        let tag: u64 = rand::random();
        format!("socks5h://anonimax{tag}:x@{}", self.socks)
    }
}

pub async fn reachable(socks: &str) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(socks)).await,
        Ok(Ok(_))
    )
}

pub async fn new_identity(control: &str) -> anyhow::Result<()> {
    let mut stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(control))
        .await
        .map_err(|_| anyhow::anyhow!("control port {control} not reachable"))??;

    stream.write_all(b"AUTHENTICATE\r\n").await?;
    let reply = read_reply(&mut stream).await?;
    if !reply.starts_with("250") {
        anyhow::bail!("control auth rejected: {}", reply.trim());
    }

    stream.write_all(b"SIGNAL NEWNYM\r\n").await?;
    let reply = read_reply(&mut stream).await?;
    if !reply.starts_with("250") {
        anyhow::bail!("NEWNYM rejected: {}", reply.trim());
    }

    let _ = stream.write_all(b"QUIT\r\n").await;
    Ok(())
}

async fn read_reply(stream: &mut TcpStream) -> anyhow::Result<String> {
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf).await?;
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}
