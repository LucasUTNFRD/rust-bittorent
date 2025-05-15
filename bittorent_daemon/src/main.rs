use anyhow::{Context, Result};
use bittorrent_core::{torrent_parser::TorrentParser, torrent_session::Client};
use std::{env, path::Path};
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let torrent_path = env::args()
        .nth(1)
        .context("usage: program <torrent-file>")?;
    let torrent_path = Path::new(&torrent_path);

    let torrent = TorrentParser::parse(torrent_path)
        .with_context(|| format!("failed to parse torrent file: {:?}", torrent_path))?;

    let client = Client::new();
    client.add_torrent(torrent);

    println!("Press Ctrl+C to stop...");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c()
        .await
        .context("failed to listen for Ctrl+C")?;

    info!("Received Ctrl+C, shutting down.");

    Ok(())
}
