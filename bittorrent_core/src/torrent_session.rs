use std::{collections::HashMap, sync::Arc};

use tokio::sync::mpsc;
use tracing::info;

use crate::{disk_io::DiskHandle, metainfo::Torrent, types::InfoHash};

struct TorrentSession {
    torrent: Arc<Torrent>,
    receiver: mpsc::Receiver<Message>,
    // piece_manager: PieceManagerActor
    // peer_list
    // tracker
    // status
}

enum Message {}

struct SessionHandle {
    sender: mpsc::Sender<Message>,
}

impl SessionHandle {
    pub fn new(torrent: Arc<Torrent>) -> Self {
        let (sender, receiver) = mpsc::channel(32);
        let session_handle = TorrentSession::new(torrent, receiver);
        tokio::task::spawn(async move { TorrentSession::run(session_handle).await });
        Self { sender }
    }
}

impl TorrentSession {
    pub fn new(torrent: Arc<Torrent>, receiver: mpsc::Receiver<Message>) -> Self {
        // start tracker

        // start listener

        //start event loop

        Self { torrent, receiver }
    }
    pub async fn run(mut session: TorrentSession) {
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

impl Client {
    pub fn new(/* setting:SessionSettings */) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        tokio::task::spawn(async move {
            let mut torrents: HashMap<InfoHash, SessionHandle> = HashMap::new();

            // start disk actor
            let disk_handle = DiskHandle::new();

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    ClientCommand::AddTorrent(torrent) => {
                        info!("starting session for TORRENT={:#?}", torrent);
                        let torrent = Arc::new(torrent);
                        let info_hash = torrent.info_hash;
                        let torrent_handler = SessionHandle::new(torrent);
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
