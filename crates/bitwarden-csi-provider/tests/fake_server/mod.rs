//! Helper to start the bitwarden fake-server for integration tests.
//!
//! The fake-server binary must be pre-built and available at one of:
//! 1. `BWS_FAKE_SERVER_BIN` environment variable
//! 2. `./target/debug/fake-server` (if built locally)
//!
//! If not found, tests that require it will be skipped (not failed).

use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;

/// Hardcoded access token from the bitwarden fake-server.
/// See: https://github.com/bitwarden/sdk-sm/tree/main/crates/fake-server
const FAKE_SERVER_ACCESS_TOKEN: &str =
    "0.ec2c1d46-6a4b-4751-a310-af9601317f2d.C2IgxjjLF7qSshsbwe8JGcbM075YXw:X8vbvA0bduihIDe/qrzIQQ==";

/// Pre-seeded secret UUID in the fake-server.
/// The fake-server seeds a secret with key "FERRIS" and value "btw" at this UUID.
const SEEDED_SECRET_ID: &str = "15744a66-341a-4c62-af56-b15800cf6fa1";

pub struct FakeServer {
    process: Option<Child>,
    port: u16,
    available: bool,
}

impl FakeServer {
    pub async fn start() -> Self {
        let port = find_available_port();

        let bin_path = std::env::var("BWS_FAKE_SERVER_BIN")
            .ok()
            .or_else(|| {
                // Check common locations
                let candidates = [
                    "fake-server",
                    "./target/debug/fake-server",
                    "../../../fake-server/target/debug/fake-server",
                ];
                candidates
                    .iter()
                    .find(|p| std::path::Path::new(p).exists())
                    .map(|s| s.to_string())
            });

        let Some(bin) = bin_path else {
            eprintln!("fake-server binary not found — integration tests will be skipped");
            return Self {
                process: None,
                port,
                available: false,
            };
        };

        let child = Command::new(&bin)
            .env("SM_FAKE_SERVER_PORT", port.to_string())
            .env("RUST_LOG", "warn")
            .spawn();

        match child {
            Ok(child) => {
                // Wait for the server to be ready
                let ready = wait_for_port(port, Duration::from_secs(10)).await;
                if !ready {
                    eprintln!("fake-server failed to start on port {port}");
                    return Self {
                        process: Some(child),
                        port,
                        available: false,
                    };
                }

                Self {
                    process: Some(child),
                    port,
                    available: true,
                }
            }
            Err(e) => {
                eprintln!("failed to spawn fake-server at {bin}: {e}");
                Self {
                    process: None,
                    port,
                    available: false,
                }
            }
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    pub fn access_token(&self) -> &str {
        FAKE_SERVER_ACCESS_TOKEN
    }

    pub fn seeded_secret_id(&self) -> &str {
        SEEDED_SECRET_ID
    }

    pub fn is_available(&self) -> bool {
        self.available
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn find_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind to random port");
    listener.local_addr().unwrap().port()
}

async fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}
