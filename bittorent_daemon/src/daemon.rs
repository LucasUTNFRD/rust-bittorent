use std::{
    collections::HashMap,
    env,
    fs::remove_file,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use bittorrent_core::torrent_parser::TorrentParser;
use tokio::{
    io::AsyncReadExt,
    net::{UnixListener, UnixStream},
    spawn,
    sync::mpsc,
    task::JoinHandle,
};
use tracing::{error, info, warn};

use crate::{config::Settings, error::Error, torrent_session::TorrentSession};

struct SessionHanlde {
    join_hanlde: JoinHandle<()>,
    // sesssion_tx: mpsc::Sender<TorrentSessionMsg>,
}

type TorrentId = String;

pub struct Daemon {
    client_cfg: Settings,
    sessions: HashMap<TorrentId, SessionHanlde>,
}

const SOCKET_PATH: &str = "/tmp/bittorent-protocol.tmp";

/// Message types for main client controller
pub enum ClientMsg {}

impl Daemon {
    pub fn new(client_cfg: Settings) -> Self {
        Self {
            client_cfg,
            sessions: HashMap::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
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
        let listener = UnixListener::bind(SOCKET_PATH).unwrap();
        info!("Daemon listening on {}", SOCKET_PATH);

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

    async fn handle_connection(&mut self, mut stream: UnixStream) -> Result<()> {
        let config = config::standard();

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
        let (msg, _consumed): (DaemonMsg, usize) = bincode::decode_from_slice(&msg_buf, config)
            .context("Failed to deserialize DaemonMsg")?;

        info!("Message recv:{:?}", &msg);

        self.handle_message(msg).await.unwrap();

        Ok(())
    }

    async fn handle_message(&mut self, msg: DaemonMsg) -> Result<(), Error> {
        match msg {
            DaemonMsg::AddTorrent(torrent_filename) => {
                // info!("Torrent dir as string:{torrent_dir}");
                // let mut filepath = PathBuf::from();
                // let path = Path::new(&torrent_dir);
                // filepath.push(path);
                // if !filepath.exists() {
                //     warn!("Non-existing file{}");
                //     return Err(Error::InvalidTorrent);
                // }
                let base_dir = PathBuf::from("./sample_torrents");
                let torrent_path = base_dir.join(&torrent_filename); // Join base dir and filename

                info!("Looking for torrent file at: {:?}", torrent_path);

                // Check if the constructed path exists
                if !torrent_path.exists() {
                    error!("Torrent file does not exist at path: {:?}", torrent_path);
                    // Return a specific error indicating the file wasn't found where expected
                    // You might want to add a new variant to your Error enum for this
                    return Err(Error::InvalidTorrent); // Or a more specific error
                }
                if !torrent_path.is_file() {
                    error!("Specified path is not a file: {:?}", torrent_path);
                    return Err(Error::InvalidTorrent); // Or a more specific error
                }

                let torrent_file = Arc::new(TorrentParser::parse(&torrent_path).unwrap());
                let mut session = TorrentSession::new(
                    torrent_file,
                    self.client_cfg.save_directory.clone(),
                    self.client_cfg.listen_port,
                );

                spawn(async move {
                    let _ = session.start_running_session().await;
                    Ok::<(), Error>(())
                });

                Ok(())
            }
        }
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
