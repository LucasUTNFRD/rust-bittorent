use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};

use bytes::Bytes;
use rand::Rng;
use sha1::{Digest, Sha1};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, info, warn};

use crate::{
    bitfield::{self, BitField},
    disk_io::{DiskHandle, IOMessage},
    metainfo::TorrentInfo,
    peer::PeerInfo,
    piece_cache::PieceCache,
    piece_picker::{Block, BlockInfo, PiecePicker},
    tracker::tracker_client::TrackerClient,
    types::{InfoHash, PeerId},
};

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

// REFACTOR:
// PeerBitfield and PeerHave could be merged into one message
// of type PeerUpdate which is either a bitfield of a piece acquired
// eitehr of both update could send us an oneshot channel where we can send if we are interested or not.
pub enum TorrentMessage {
    PeerList(Vec<SocketAddr>),
    PeerConnected(SocketAddr),
    PeerDisconnected(SocketAddr),
    PeerBitfield {
        peer_addr: SocketAddr,
        peer_bf: BitField,
        interest: oneshot::Sender<bool>,
    },
    PeerHave(SocketAddr, u32, Option<oneshot::Sender<bool>>),
    Piece(SocketAddr, Block),
    GetTask(SocketAddr, oneshot::Sender<Option<Vec<BlockInfo>>>),
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
        // Suscribe to disk io
        let info_hash = session.torrent.info_hash;
        let name = session.torrent.info.name.clone();
        let file_size = session.torrent.info.length as u64;
        session
            .disk_handler
            .register_torrent(info_hash, name, file_size)
            .await;

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
                    for addr in peers.iter() {
                        if !session.peer_list.contains(&addr) {
                            session.connect_to_peer(*addr)
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
                    session.piece_manager.unregister_peer(&addr);
                    session.peer_list.remove(&addr);
                    // BUG: When peer disconnets while is trying to download all the piece that we didnt receiv should be en-queued
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
                    // Track stats of amount of downloaded for this peer
                    if let Some(piece_completed) = session.piece_cache.insert_block(block) {
                        debug!("Piece completed {}", piece_completed.0);
                        session.try_write_piece(piece_completed, peer_addr).await;
                    }
                }
                TorrentMessage::GetTask(peer_addr, task_sender) => {
                    let piece_to_request = session.piece_manager.pick_piece(&peer_addr);
                    if piece_to_request.is_none() {
                        info!("[{}] No more piece to request from this peer", peer_addr);
                    }
                    if let Err(_e) = task_sender.send(piece_to_request) {
                        warn!("Failed sending tasks we found for peer");
                    }
                }
                TorrentMessage::PeerHave(peer_addr, piece_idx, resp) => {
                    debug!("Handling peer have");
                    session.piece_manager.update_peer(&peer_addr, piece_idx);
                    if let Some(interest) = resp {
                        let am_interested = session.piece_manager.check_interest(peer_addr);
                        let _ = interest
                            .send(am_interested)
                            .expect("Interes resp oneshot chan closed");
                    }
                }
            }
        }
    }

    // TODO: Move this logic to a dedicated actor
    async fn piece_validation(&self, piece_idx: usize, piece_data: Bytes) -> bool {
        let expected_hash = self
            .torrent
            .info
            .pieces
            .get(piece_idx)
            .ok_or_else(|| warn!("Invalid piece index: {}", piece_idx))
            .expect("Valid piece index")
            .clone();

        // Spawn blocking task for hash validation
        let data_clone = piece_data.clone();
        let hash_valid = tokio::task::spawn_blocking(move || {
            let mut hasher = Sha1::new();
            hasher.update(&data_clone);
            let computed_hash: [u8; 20] = hasher.finalize().into();
            computed_hash == expected_hash.0
        })
        .await
        .unwrap_or(false);

        hash_valid
    }

    async fn try_write_piece(&mut self, piece: (usize, Bytes), peer: SocketAddr) {
        let piece_idx = piece.0;
        let piece_data = piece.1;

        // Get expected hash from torrent metadata
        if !self.piece_validation(piece_idx, piece_data.clone()).await {
            warn!(
                "Invalid piece {} from peer {} it should be marked to not requested",
                piece_idx, peer
            );
            // BUG: Re-queue piece or penalize peer
            return;
        }

        // Proceed to write validated data
        let offset: u64 = piece_idx as u64 * self.torrent.info.piece_length as u64;
        self.disk_handler
            .send(IOMessage::WriteBlock {
                info_hash: self.torrent.info_hash,
                offset,
                data: piece_data,
            })
            .await;

        self.piece_manager.mark_piece_downloaded(piece_idx);
        self.bitfield.set_piece(piece_idx);
        // TODO: Broadcast Have piece
    }

    fn connect_to_peer(&self, addr: SocketAddr) {
        let tx = self.cmd_tx.clone();
        let our_id = self.our_id;
        let info_hash = self.torrent.info_hash;
        let torrent = self.torrent.clone();
        let bitfield = if self.bitfield.is_empty() {
            None
        } else {
            Some(self.bitfield.clone())
        };
        tokio::task::spawn(async move {
            match PeerInfo::try_connect_to_peer(&addr, our_id, info_hash).await {
                Ok(stream) => {
                    let _ = tx.send(TorrentMessage::PeerConnected(addr)).await;
                    if let Err(e) = PeerInfo::new(addr, tx.clone(), torrent)
                        .start(stream, bitfield)
                        .await
                    {
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
