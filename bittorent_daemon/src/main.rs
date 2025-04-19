use anyhow::Result;
use bittorent_daemon::daemon::Daemon;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup tracing/logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting BitTorrent Daemon...");

    // TODO: Load configuration
    // TODO: Parse command-line arguments (if feature enabled)
    // TODO: Start the main DaemonSupervisor actor
    // TODO: Setup IPC listener (e.g., Unix Domain Socket)
    //

    Daemon::run().await;

    // Placeholder: Wait forever or until shutdown signal
    // tokio::signal::ctrl_c().await?;
    // info!("Shutting down daemon...");

    // TODO: Implement graceful shutdown logic

    Ok(())
}
