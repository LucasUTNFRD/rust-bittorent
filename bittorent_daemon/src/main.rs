use std::path::PathBuf;

use anyhow::Result;
use bittorent_daemon::{
    config::{
        Settings, default_listen_port, default_max_peers, default_save_directory,
        default_socket_path,
    },
    daemon::Daemon,
};
use tracing::info;

pub struct ClientSettings {
    pub listen_port: u16,
    pub max_peer_connections_per_torrent: usize,
    pub save_directory: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup tracing/logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting BitTorrent Daemon...");

    // TODO: Load configuration
    let settings = match Settings::new() {
        Ok(s) => {
            info!("Configuration loaded successfully: {:?}", s);
            s
        }
        Err(e) => {
            // Log error and potentially exit or use hardcoded defaults
            eprintln!("Failed to load configuration: {}", e);
            // Decide how to handle this - exit or use safe defaults
            // For now, let's use the struct's defaults by creating manually
            eprintln!("Using default settings.");
            Settings {
                listen_port: default_listen_port(),
                save_directory: default_save_directory(),
                socket_path: default_socket_path(),
                max_peer_connections_per_torrent: default_max_peers(),
                // ... ensure all fields covered by defaults ...
            }
            // Alternatively, exit:
            // return Err(anyhow::anyhow!("Configuration error: {}", e));
        }
    };
    // TODO: Parse command-line arguments (if feature enabled)
    // TODO: Start the main DaemonSupervisor actor
    // TODO: Setup IPC listener (e.g., Unix Domain Socket)
    //
    //

    let mut daemon = Daemon::new(settings.clone());
    let _ = daemon.run().await;

    // Placeholder: Wait forever or until shutdown signal
    // tokio::signal::ctrl_c().await?;
    // info!("Shutting down daemon...");

    // TODO: Implement graceful shutdown logic

    Ok(())
}
