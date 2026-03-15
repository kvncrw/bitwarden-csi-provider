mod config;
mod health;
mod server;

use std::fs;

use bws_csi_proto::v1alpha1::csi_driver_provider_server::CsiDriverProviderServer;
use clap::Parser;
use tokio::net::UnixListener;
use tokio::signal;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing::info;

use config::Config;
use server::BwsCsiProviderService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse();

    // Initialize tracing
    match config.log_format.as_str() {
        "json" => {
            tracing_subscriber::fmt().json().init();
        }
        _ => {
            tracing_subscriber::fmt().init();
        }
    }

    let socket_path = config.socket_path();

    // Clean up stale socket
    if socket_path.exists() {
        info!(path = %socket_path.display(), "removing stale socket");
        fs::remove_file(&socket_path)?;
    }

    // Ensure provider directory exists
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let uds = UnixListener::bind(&socket_path)?;
    let uds_stream = UnixListenerStream::new(uds);

    info!(
        socket = %socket_path.display(),
        version = env!("CARGO_PKG_VERSION"),
        "bws-csi-provider starting"
    );

    // Spawn health probe server
    let health_socket_path = socket_path.clone();
    tokio::spawn(health::serve_health(
        config.health_addr,
        health_socket_path,
    ));

    // Run gRPC server until SIGTERM/SIGINT
    Server::builder()
        .add_service(CsiDriverProviderServer::new(BwsCsiProviderService))
        .serve_with_incoming_shutdown(uds_stream, async {
            let ctrl_c = signal::ctrl_c();
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to register SIGTERM handler");

            tokio::select! {
                _ = ctrl_c => info!("received SIGINT, shutting down"),
                _ = sigterm.recv() => info!("received SIGTERM, shutting down"),
            }
        })
        .await?;

    // Cleanup socket on graceful shutdown
    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
        info!(path = %socket_path.display(), "socket removed");
    }

    Ok(())
}
