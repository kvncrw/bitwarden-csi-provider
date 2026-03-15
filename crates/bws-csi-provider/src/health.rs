use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{info, warn};

/// Simple HTTP health probe server.
/// Returns 200 if the provider socket file exists, 503 otherwise.
pub async fn serve_health(addr: String, socket_path: PathBuf) {
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            info!(addr = %addr, "health probe server started");
            l
        }
        Err(e) => {
            warn!(error = %e, addr = %addr, "failed to bind health probe server");
            return;
        }
    };

    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };

        let (status, body) = if socket_path.exists() {
            ("200 OK", "ok")
        } else {
            ("503 Service Unavailable", "socket not found")
        };

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{body}",
            body.len()
        );

        let _ = stream.write_all(response.as_bytes()).await;
    }
}
