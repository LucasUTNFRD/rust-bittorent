use std::{fs::remove_file, path::Path};

use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use tokio::{
    io::AsyncReadExt,
    net::{UnixListener, UnixStream},
};
use tracing::{error, info, warn};

pub struct Daemon {}

const SOCKET_PATH: &str = "/tmp/bittorent-protocol.tmp";

impl Daemon {
    // pub fn new() -> Self {
    //     todo!()
    // }
    pub async fn run() -> Result<()> {
        let sock_path = Path::new(SOCKET_PATH);
        if sock_path.exists() {
            match remove_file(sock_path) {
                Ok(_) => {
                    info!("the path was already used, now i remove it");
                }
                Err(_) => {
                    warn!("Err");
                }
            }
        }
        let listener = UnixListener::bind(SOCKET_PATH).unwrap();
        info!("Daemon listening on {}", SOCKET_PATH);

        // 3. Accept connections in a loop
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    info!("Accepted new connection");
                    // Spawn a new task to handle each connection concurrently
                    Self::handle_connection(stream).await.unwrap();
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    // Depending on the error, you might want to break the loop or just log
                }
            }
        }
    }

    async fn handle_connection(mut stream: UnixStream) -> Result<()> {
        let config = config::standard(); // Bincode configuration

        // Read message length (4 bytes, big-endian u32)
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .context("Failed to read message length from stream")?;
        let msg_len = u32::from_be_bytes(len_buf) as usize;
        // debug!("Received message length: {}", msg_len);

        // Read the actual message bytes
        let mut msg_buf = vec![0u8; msg_len];
        stream
            .read_exact(&mut msg_buf)
            .await
            .context("Failed to read message body from stream")?;
        // debug!("Read {} message bytes", msg_buf.len());

        // Deserialize the message
        let (msg, consumed): (DaemonMsg, usize) = bincode::decode_from_slice(&msg_buf, config)
            .context("Failed to deserialize DaemonMsg")?;

        println!("Message recv:{:?}", msg);

        Ok(())
    }
}

#[derive(Debug, Encode, Decode, Clone)] // Add Clone if needed later
pub enum DaemonMsg {
    AddTorrent(String),
    Status(String),
    List {
        active_only: bool,
        completed_only: bool,
    },
}

#[derive(Debug, Encode, Decode, Clone)] // Add Clone if needed later
pub enum DaemonResponse {
    Success(String),
    TorrentList(Vec<TorrentInfo>),
    TorrentStatus(TorrentInfo),
    Error(String),
}

#[derive(Debug, Encode, Decode, Clone)] // Add Clone if needed later
pub struct TorrentInfo {
    id: String,
    name: String,
    progress: f32,
    status: TorrentStatus,
    // Add more fields as needed
}

#[derive(Debug, Encode, Decode, Clone, PartialEq)] // Add PartialEq for filtering
pub enum TorrentStatus {
    Downloading,
    Seeding,
    Paused,
    Completed,
}
