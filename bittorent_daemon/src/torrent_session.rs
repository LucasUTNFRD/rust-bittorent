use std::{
    collections::HashMap,
    net::SocketAddrV4,
    sync::{Arc, RwLock},
};

use tracing::{debug, info};

use bittorrent_core::{metainfo::Torrent, types::PeerId};
use tokio::{
    select,
    sync::mpsc::{self, Sender},
    time::{Instant, interval_at},
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
    fn new(session_rx: mpsc::Receiver<TorrentSessionMsg>, torrent: Arc<Torrent>) -> Self {
        Self {
            session_rx,
            torrent,
            active_peers: RwLock::new(HashMap::new()),
            tracker_ref: None,
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
        for peer in peers {
            info!("Should connect to Peer{:?}", peer);
            // spawn a tokio task
            // connect with the peer
            // try to perform a handhsake
            // tokio::spawn()
        }
    }
}

impl SessionHanlde {
    pub fn new(torrent: Arc<Torrent>) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let torrent_session = TorrentSession::new(rx, torrent);
        tokio::spawn(torrent_session.start());

        Self { sender: tx }
    }
}
