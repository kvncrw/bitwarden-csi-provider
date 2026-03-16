use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::{info, warn};

/// Simple HTTP health probe server.
/// Returns 200 on /healthz if the provider socket file exists, 503 otherwise.
/// Returns 404 for any other path.
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

        // Read the request line to extract the path
        let mut reader = BufReader::new(&mut stream);
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).await.is_err() {
            continue;
        }

        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("");

        let (status, body) = match path {
            "/healthz" | "/health" | "/livez" | "/readyz" => {
                if socket_path.exists() {
                    ("200 OK", "ok")
                } else {
                    ("503 Service Unavailable", "socket not found")
                }
            }
            _ => ("404 Not Found", "not found"),
        };

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{body}",
            body.len()
        );

        let _ = stream.write_all(response.as_bytes()).await;
    }
}
