use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};

use bytes::Bytes;
use rand::Rng;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use crate::{
    bitfield::BitField,
    disk_io::{DiskHandle, IOMessage},
    metainfo::TorrentInfo,
    peer::PeerInfo,
    piece_cache::PieceCache,
    piece_picker::{Block, PiecePicker},
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
    // bitfield:,
    /// channel for receiving messages from the client
    cmd_rx: mpsc::Receiver<TorrentMessage>,
    cmd_tx: mpsc::Sender<TorrentMessage>,

    disk_handler: Arc<DiskHandle>,
    our_id: PeerId,
    piece_manager: PiecePicker,
    peer_list: HashSet<SocketAddr>,
    bitfield: BitField,
    piece_cache: PieceCache,
}

#[derive(Clone)]
struct TorrentHandle {
    sender: mpsc::Sender<TorrentMessage>,
}

pub enum TorrentMessage {
    PeerList(Vec<SocketAddr>),
    PeerConnected(SocketAddr),
    PeerDisconnected(SocketAddr),
    PeerBitfield {
        peer_addr: SocketAddr,
        peer_bf: BitField,
        interest: oneshot::Sender<bool>,
    },
    Piece(SocketAddr, Block),
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
        let num_pieces = torrent.get_total_pieces();
        let piece_manager = PiecePicker::from(torrent.clone());
        let piece_cache = PieceCache::from(torrent.clone());
        Self {
            torrent,
            cmd_rx: receiver,
            disk_handler,
            our_id: client_id,
            cmd_tx,
            peer_list: HashSet::new(),
            bitfield: BitField::new(num_pieces),
            piece_manager,
            piece_cache,
        }
    }
    pub async fn run(mut session: Torrent) {
        let our_id = session.our_id;
        let cmd_tx = session.cmd_tx.clone();

        //spawn tracker task
        {
            let torrent = Arc::clone(&session.torrent);
            let mut tracker = TrackerClient::new(torrent, our_id, cmd_tx);
            tokio::task::spawn(async move {
                tracker.start().await;
            });
        }

        while let Some(msg) = session.cmd_rx.recv().await {
            match msg {
                TorrentMessage::PeerList(peers) => {
                    info!("Received {:?} peers from tracker", peers);
                    for addr in peers {
                        if !session.peer_list.contains(&addr) {
                            session.connect_to_peer(addr)
                        }
                    }
                }
                TorrentMessage::PeerConnected(addr) => {
                    info!("Peer connected");
                    session.peer_list.insert(addr);
                }
                TorrentMessage::PeerDisconnected(addr) => {
                    // when a peer disconnects in the middle while we leech the file
                    // we need to re-update the tracked pieces subsrtacting the availability of
                    // pieces which disconnected peer had
                    info!("Peer disconnected");
                    //session.piece_picker.decrement(addr);
                    session.peer_list.remove(&addr);
                }
                TorrentMessage::PeerBitfield {
                    peer_addr,
                    peer_bf,
                    interest,
                } => {
                    session.piece_manager.register_peer(peer_addr, peer_bf);
                    let am_interested = session.piece_manager.check_interest(peer_addr);
                    let _ = interest.send(am_interested);
                }
                TorrentMessage::Piece(peer_addr, block) => {
                    if let Some(piece_completed) = session.piece_cache.insert_block(block) {
                        session.try_write_piece(piece_completed, peer_addr);
                    }
                }
            }
        }
    }

    fn try_write_piece(&self, piece: (usize, Bytes), peer: SocketAddr) {
        // validate it
        // if not valid
        // rank peer as possible malicious
        // if valid then
        // write piece to disk
    }

    fn connect_to_peer(&self, addr: SocketAddr) {
        let tx = self.cmd_tx.clone();
        let our_id = self.our_id;
        let info_hash = self.torrent.info_hash;
        let torrent = self.torrent.clone();
        tokio::task::spawn(async move {
            match PeerInfo::try_connect_to_peer(&addr, our_id, info_hash).await {
                Ok(stream) => {
                    let _ = tx.send(TorrentMessage::PeerConnected(addr)).await;
                    if let Err(e) = PeerInfo::new(addr, tx.clone(), torrent).start(stream).await {
                        let _ = tx.send(TorrentMessage::PeerDisconnected(addr)).await;
                        warn!("Peer [{}] Disconnected with error {}", addr, e);
                    }
                }
                Err(e) => {
                    let _ = tx.send(TorrentMessage::PeerDisconnected(addr)).await;
                    warn!("connection to {:?} failed: {:?}", addr, e);
                }
            };
        });
    }

    //returns a vector with information about pieces that are partially downloaded or not downloaded but partially requested
    fn get_download_queue() -> Option<Vec<Block>> {
        todo!()
    }

    fn read_piece() {}

    fn write_piece() {}
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
