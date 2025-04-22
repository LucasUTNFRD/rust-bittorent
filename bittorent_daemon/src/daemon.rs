use std::{
    collections::HashMap,
    fs::remove_file,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use bittorrent_core::{metainfo::Torrent, torrent_parser::TorrentParser, types::InfoHash};
use tokio::{
    io::AsyncReadExt,
    net::{UnixListener, UnixStream},
};
use tracing::{error, info, warn};

use crate::{
    config::Settings,
    error::ClientError,
    torrent_session::{SessionHanlde, TorrentSession},
};

type TorrentId = String;

pub struct Daemon {
    client_cfg: Settings,
    sessions: HashMap<InfoHash, SessionHanlde>,
}

/// Message types for main client controller
pub enum ClientMsg {}

impl Daemon {
    pub fn new(client_cfg: Settings) -> Self {
        Self {
            client_cfg,
            sessions: HashMap::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), ClientError> {
        let sock_path = Path::new(&self.client_cfg.socket_path);
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
        let listener = UnixListener::bind(sock_path).unwrap();
        info!("Daemon listening on {:?}", &sock_path);

        // 3. Accept connections in a loop
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    info!("Accepted new connection");
                    // Spawn a new task to handle each connection concurrently
                    self.handle_connection(stream).await?
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    // Depending on the error, you might want to break the loop or just log
                }
            }
        }
    }

    async fn handle_connection(&mut self, mut stream: UnixStream) -> Result<(), ClientError> {
        let config = config::standard();

        // Read message length (4 bytes, big-endian u32)
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(ClientError::IO)?;

        let msg_len = u32::from_be_bytes(len_buf) as usize;
        // debug!("Received message length: {}", msg_len);

        // Read the actual message bytes
        let mut msg_buf = vec![0u8; msg_len];
        stream
            .read_exact(&mut msg_buf)
            .await
            .map_err(ClientError::IO)?;
        // debug!("Read {} message bytes", msg_buf.len());

        // Deserialize the message
        let (msg, _consumed): (DaemonMsg, usize) =
            bincode::decode_from_slice(&msg_buf, config).map_err(ClientError::Decode)?;

        info!("Message recv:{:?}", &msg);

        self.handle_message(msg).await.unwrap();

        Ok(())
    }

    async fn handle_message(&mut self, msg: DaemonMsg) -> Result<(), ClientError> {
        match msg {
            DaemonMsg::AddTorrent(torrent_filename) => {
                let _ = self.add_torrent(&torrent_filename).await;
            }
        }
        Ok(())
    }

    async fn add_torrent(&mut self, filename: &str) -> Result<(), ClientError> {
        let base_dir = PathBuf::from("./sample_torrents");
        let torrent_path = base_dir.join(filename); // Join base dir and filename

        info!("Looking for torrent file at: {:?}", torrent_path);

        if !torrent_path.exists() {
            error!("Torrent file does not exist at path: {:?}", torrent_path);
            return Err(ClientError::InvalidTorrent);
        }
        if !torrent_path.is_file() {
            error!("Specified path is not a file: {:?}", torrent_path);
            return Err(ClientError::InvalidTorrent);
        }

        let parsed_torrent =
            TorrentParser::parse(&torrent_path).map_err(ClientError::TorrentParsing)?;

        let torrent_file = Arc::new(parsed_torrent);

        let session_handle = SessionHanlde::new(torrent_file.clone());
        self.sessions.insert(torrent_file.info_hash, session_handle);

        Ok(())
    }
}

#[derive(Debug, Encode, Decode, Clone)] // Add Clone if needed later
pub enum DaemonMsg {
    AddTorrent(String),
}

#[derive(Debug, Encode, Decode, Clone)] // Add Clone if needed later
pub enum DaemonResponse {
    Success(String),
}
