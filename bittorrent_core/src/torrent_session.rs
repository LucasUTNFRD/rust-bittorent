use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use rand::Rng;
use tokio::{net::TcpStream, sync::mpsc};
use tracing::{debug, info, warn};

use crate::{
    disk_io::DiskHandle,
    metainfo::TorrentInfo,
    peer::PeerInfo,
    tracker::tracker_client::TrackerClient,
    types::{InfoHash, PeerId},
};

// ---- Torrent ----
// Internal logic for managing torrent

/// Core structure for orchestration of:
/// - Read/Write pieces to/from disk
/// - Manage Peers
/// - Track piece availability
/// - Stats
struct Torrent {
    torrent: Arc<TorrentInfo>,
    /// channel for receiving messages from the client
    cmd_rx: mpsc::Receiver<TorrentMessage>,
    cmd_tx: mpsc::Sender<TorrentMessage>,

    disk_handler: Arc<DiskHandle>,
    our_id: PeerId,
    // piece_manager: PieceManagerActor
    // peer_list
    peer_list: HashSet<SocketAddr>,
    // tracker
    // status
}

#[derive(Clone)]
struct TorrentHandle {
    sender: mpsc::Sender<TorrentMessage>,
}

pub enum TorrentMessage {
    PeerList(Vec<SocketAddr>),
    PeerConnected,
    PeerDisconnected,
}

impl TorrentHandle {
    pub fn new(torrent: Arc<TorrentInfo>, disk_handle: Arc<DiskHandle>, client_id: PeerId) -> Self {
        let (sender, receiver) = mpsc::channel(10000);
        let session_handle =
            Torrent::new(torrent, receiver, disk_handle, client_id, sender.clone());
        tokio::task::spawn(async move { Torrent::run(session_handle).await });
        Self { sender }
    }
}

impl Torrent {
    pub fn new(
        torrent: Arc<TorrentInfo>,
        receiver: mpsc::Receiver<TorrentMessage>,
        disk_handler: Arc<DiskHandle>,
        client_id: PeerId,
        cmd_tx: mpsc::Sender<TorrentMessage>,
    ) -> Self {
        // We should start the piece picker
        Self {
            torrent,
            cmd_rx: receiver,
            disk_handler,
            our_id: client_id,
            cmd_tx,
            peer_list: HashSet::new(),
        }
    }
    pub async fn run(mut session: Torrent) {
        let torrent = Arc::clone(&session.torrent);
        let mut tracker = TrackerClient::new(torrent, session.our_id, session.cmd_tx);
        tokio::task::spawn(async move {
            tracker.start().await;
        });

        while let Some(msg) = session.cmd_rx.recv().await {
            match msg {
                TorrentMessage::PeerList(peers) => {
                    let info_hash = session.torrent.info_hash;
                    info!("Received {:?} peers from tracker", peers);
                    for addr in peers {
                        // let info_hash = torrent.info_hash;
                        if !session.peer_list.contains(&addr) {
                            tokio::task::spawn(async move {
                                match PeerInfo::try_connect_to_peer(
                                    &addr,
                                    session.our_id,
                                    info_hash,
                                )
                                .await
                                {
                                    Ok(stream) => {
                                        if let Err(e) = PeerInfo::new(addr).start(stream).await {
                                            warn!("Peer [{}] Disnconnected with error {}", addr, e);
                                        }
                                    }
                                    Err(e) => {
                                        warn!("connect to {:?} failed: {:?}", addr, e);
                                    }
                                };
                            });
                        }
                    }
                }
                TorrentMessage::PeerConnected => {
                    info!("Peer connected");
                }
                TorrentMessage::PeerDisconnected => {
                    info!("Peer disconnected");
                }
            }
        }
    }
}

// ---- Client ----
pub struct Client {
    cmd_tx: mpsc::UnboundedSender<ClientCommand>,
    // settings: SessionSettings,
    // disk_io: DiskIOActor,
}

enum ClientCommand {
    AddTorrentInfo(TorrentInfo),
    // Pause,
    // Resume,
}

impl Default for Client {
    fn default() -> Self {
        Client::new()
    }
}

fn generate_peer_id() -> PeerId {
    let mut peer_id = [0u8; 20];
    peer_id[0..3].copy_from_slice(b"-RS"); // Client identifier
    rand::rng().fill(&mut peer_id[3..]); // Random bytes
    PeerId(peer_id)
}

impl Client {
    pub fn new(/* setting:SessionSettings */) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        tokio::task::spawn(async move {
            let mut torrents: HashMap<InfoHash, TorrentHandle> = HashMap::new();

            //Client ID
            let client_id = generate_peer_id();

            // start disk actor
            let disk_handle = Arc::new(DiskHandle::new());

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    ClientCommand::AddTorrentInfo(torrent) => {
                        let torrent = Arc::new(torrent);
                        let info_hash = torrent.info_hash;
                        let torrent_handler =
                            TorrentHandle::new(torrent, disk_handle.clone(), client_id);
                        torrents.insert(info_hash, torrent_handler);
                    }
                }
            }
        });

        Self { cmd_tx }
    }

    pub fn add_torrent(&self, torrent: TorrentInfo) {
        let _ = self.cmd_tx.send(ClientCommand::AddTorrentInfo(torrent));
    }
}
