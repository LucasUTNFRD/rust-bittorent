use std::{
    collections::HashMap,
    net::SocketAddrV4,
    sync::{Arc, RwLock, atomic::AtomicUsize},
    time::Duration,
};

use anyhow::Ok;
use tracing::{debug, info, warn};

use bittorrent_core::{metainfo::Torrent, types::PeerId};
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc::{self, Sender},
    time::{Instant, interval_at, timeout},
};

use crate::{
    peer_connection::PeerConnection,
    tracker_communication::tracker_client::{TrackerClient, TrackerMessage},
};

pub struct TorrentSession {
    /// mailbox for communication with the daemon
    session_rx: mpsc::Receiver<TorrentSessionMsg>,
    torrent: Arc<Torrent>,
    active_peers: RwLock<HashMap<SocketAddrV4, PeerConnection>>,
    // piece_manager: Arc<PieceManagerHandler>,
    tracker_ref: Option<Sender<TrackerMessage>>,
    client_peer_id: PeerId,
    active_outgoing_connections: Arc<AtomicUsize>,
    // client_config: Arc<
    // disk_io: Arc<DiskIOHandler>,
}

#[derive(Clone)]
pub struct SessionHanlde {
    sender: mpsc::Sender<TorrentSessionMsg>,
}

pub enum TorrentSessionMsg {
    Pause,
    Resume,
}

pub enum TorrentSessionError {
    Example,
}

impl TorrentSession {
    fn new(
        session_rx: mpsc::Receiver<TorrentSessionMsg>,
        torrent: Arc<Torrent>,
        client_peer_id: PeerId,
    ) -> Self {
        Self {
            session_rx,
            torrent,
            active_peers: RwLock::new(HashMap::new()),
            tracker_ref: None,
            client_peer_id,
            active_outgoing_connections: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub async fn start(mut self) -> Result<(), TorrentSessionError> {
        //1. contact tracker
        info!("Announcing to tracker");
        let (tracker, tracker_tx) = TrackerClient::new(
            self.torrent.announce.clone(),
            self.torrent.info_hash,
            6881,
            self.torrent.info.length as u64,
            self.client_peer_id,
        );

        let resp = tracker.connect().await.unwrap();

        debug!("Got from tracker {:?}", resp);

        self.tracker_ref = Some(tracker_tx);

        let mut announce_interval = interval_at(Instant::now() + resp.interval, resp.interval);
        info!(
            "Announcing to tracker in next interval at:{:?}",
            announce_interval
        );

        self.handle_outboud_peers(resp.peers).await;

        loop {
            select! {
                Some(msg) = self.session_rx.recv() => {
                    self.handle_message(msg).await?
                }
                _ = announce_interval.tick() => {
                    info!("Time to announce to Tracker Client");
                }
            }
        }
    }

    async fn handle_message(&mut self, msg: TorrentSessionMsg) -> Result<(), TorrentSessionError> {
        match msg {
            TorrentSessionMsg::Pause => todo!(),
            TorrentSessionMsg::Resume => todo!(),
        }
    }

    /// Spawn PeerConnection Actors to start torrent download
    async fn handle_outboud_peers(&self, peers: Vec<SocketAddrV4>) {
        let peer_id = self.client_peer_id;
        let info_hash = self.torrent.info_hash;
        for peer_addr in peers {
            info!("Should connect to Peer{:?}", peer_addr);
            tokio::spawn(async move {
                if let Err(e) = PeerConnection::spawn(peer_addr, peer_id, info_hash).await {
                    warn!("Failed to spawn PeerConnection: {:?}", e);
                }
            });
        }
    }
}

impl SessionHanlde {
    pub fn new(torrent: Arc<Torrent>, client_peer_id: PeerId) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let torrent_session = TorrentSession::new(rx, torrent, client_peer_id);
        tokio::spawn(torrent_session.start());

        Self { sender: tx }
    }
}
