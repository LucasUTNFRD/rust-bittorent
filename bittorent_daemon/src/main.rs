use anyhow::{Context, Result};
use bittorrent_core::{torrent_parser::TorrentParser, torrent_session::Client};
use std::{env, path::Path};

#[tokio::main]
async fn main() -> Result<()> {
    let torrent_path = env::args()
        .nth(1)
        .context("usage: program <torrent-file>")?;
    let torrent_path = Path::new(&torrent_path);

    let torrent = TorrentParser::parse(torrent_path)
        .with_context(|| format!("failed to parse torrent file: {:?}", torrent_path))?;

    println!("TORRENT={:#?}", torrent);

    let client = Client::new();
    client.add_torrent(torrent);

    // TODO:
    // put a progress bar

    // Handle Ctrl+C gracefully

    Ok(())
}
