//! Health checks: a TCP connect, or an HTTP `GET` that must answer 200 to 399
//! (`MANIFIESTO.md`, section 5). Each check gives up after one second.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::manifest::Health;

/// How long one check may take.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(1);

/// Whether the service answering on `127.0.0.1:port` is healthy.
pub async fn is_healthy(port: u16, health: &Health) -> bool {
    timeout(CHECK_TIMEOUT, check(port, health))
        .await
        .unwrap_or(false)
}

async fn check(port: u16, health: &Health) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)).await else {
        return false;
    };
    let Health::Http(path) = health else {
        return true;
    };
    let request = format!("GET {path} HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).await.is_err() {
        return false;
    }
    let mut buffer = [0_u8; 32];
    let mut filled = 0_usize;
    while filled < 12 {
        let Some(rest) = buffer.get_mut(filled..) else {
            break;
        };
        match stream.read(rest).await {
            Ok(0) | Err(_) => break,
            Ok(read) => filled = filled.saturating_add(read),
        }
    }
    buffer
        .get(..filled)
        .and_then(status_code)
        .is_some_and(|code| (200..400).contains(&code))
}

/// The status of `HTTP/1.x NNN ...`.
fn status_code(head: &[u8]) -> Option<u16> {
    let text = std::str::from_utf8(head).ok()?;
    let mut parts = text.split(' ');
    parts
        .next()
        .filter(|version| version.starts_with("HTTP/"))?;
    parts.next()?.get(..3)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    async fn serve(answer: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut sink = [0_u8; 256];
                let _ = stream.read(&mut sink).await;
                let _ = stream.write_all(answer.as_bytes()).await;
            }
        });
        port
    }

    #[tokio::test]
    async fn a_listening_port_passes_the_tcp_check() {
        let port = serve("").await;
        assert!(is_healthy(port, &Health::Tcp).await);
    }

    #[tokio::test]
    async fn a_closed_port_fails() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            listener.local_addr().unwrap().port()
        };
        assert!(!is_healthy(port, &Health::Tcp).await);
    }

    #[tokio::test]
    async fn http_accepts_2xx_and_3xx_and_rejects_the_rest() {
        let ok = serve("HTTP/1.1 200 OK\r\n\r\n").await;
        let redirect = serve("HTTP/1.1 302 Found\r\n\r\n").await;
        let broken = serve("HTTP/1.1 500 Oops\r\n\r\n").await;
        let missing = serve("HTTP/1.1 404 Not Found\r\n\r\n").await;
        let health = Health::Http("/health".to_owned());
        assert!(is_healthy(ok, &health).await);
        assert!(is_healthy(redirect, &health).await);
        assert!(!is_healthy(broken, &health).await);
        assert!(!is_healthy(missing, &health).await);
    }

    #[tokio::test]
    async fn garbage_is_not_healthy() {
        let port = serve("hello there").await;
        assert!(!is_healthy(port, &Health::Http("/".to_owned())).await);
    }

    #[test]
    fn the_status_line_is_parsed() {
        assert_eq!(status_code(b"HTTP/1.1 204 No"), Some(204));
        assert_eq!(status_code(b"HTTP/1.0 301 Mov"), Some(301));
        assert_eq!(status_code(b"nope"), None);
        assert_eq!(status_code(b"HTTP/1.1 xx"), None);
    }
}
