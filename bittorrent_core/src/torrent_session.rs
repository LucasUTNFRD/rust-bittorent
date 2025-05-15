use std::{collections::HashMap, sync::Arc};

use rand::Rng;
use tokio::sync::mpsc;
use tracing::info;

use crate::{
    disk_io::DiskHandle,
    metainfo::Torrent,
    tracker::tracker_client::TrackerClient,
    types::{InfoHash, PeerId},
};

/// Core structure for orchestration of:
/// - Read/Write pieces to/from disk
/// - Manage Peers
/// - Track piece availability
/// - Stats
struct TorrentSession {
    torrent: Arc<Torrent>,
    receiver: mpsc::Receiver<TorrentMessage>,
    disk_handler: Arc<DiskHandle>,
    our_id: PeerId,
    // piece_manager: PieceManagerActor
    // peer_list
    // tracker
    // status
}

enum TorrentMessage {}

struct SessionHandle {
    sender: mpsc::Sender<TorrentMessage>,
}

impl SessionHandle {
    pub fn new(torrent: Arc<Torrent>, disk_handle: Arc<DiskHandle>, client_id: PeerId) -> Self {
        let (sender, receiver) = mpsc::channel(32);
        let session_handle = TorrentSession::new(torrent, receiver, disk_handle, client_id);
        tokio::task::spawn(async move { TorrentSession::run(session_handle).await });
        Self { sender }
    }
}

/// core enum for event handling
pub enum TorrentEvent {}

impl TorrentSession {
    pub fn new(
        torrent: Arc<Torrent>,
        receiver: mpsc::Receiver<TorrentMessage>,
        disk_handler: Arc<DiskHandle>,
        client_id: PeerId,
    ) -> Self {
        Self {
            torrent,
            receiver,
            disk_handler,
            our_id: client_id,
        }
    }
    pub async fn run(mut session: TorrentSession) {
        info!("starting session for TORRENT={:#?}", session.torrent);
        todo!()
    }
}

pub struct Client {
    cmd_tx: mpsc::UnboundedSender<ClientCommand>,
    // settings: SessionSettings,
    // disk_io: DiskIOActor,
}

enum ClientCommand {
    AddTorrent(Torrent),
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
            let mut torrents: HashMap<InfoHash, SessionHandle> = HashMap::new();

            //Client ID
            let client_id = generate_peer_id();

            // start disk actor
            let disk_handle = Arc::new(DiskHandle::new());

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    ClientCommand::AddTorrent(torrent) => {
                        let torrent = Arc::new(torrent);
                        let info_hash = torrent.info_hash;
                        let torrent_handler =
                            SessionHandle::new(torrent, disk_handle.clone(), client_id);
                        torrents.insert(info_hash, torrent_handler);
                    }
                }
            }
        });

        Self { cmd_tx }
    }

    pub fn add_torrent(&self, torrent: Torrent) {
        let _ = self.cmd_tx.send(ClientCommand::AddTorrent(torrent));
    }
}
