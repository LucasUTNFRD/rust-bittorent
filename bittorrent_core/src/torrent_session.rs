use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::info;

use crate::metainfo::Torrent;

pub struct TorrentSession {
    torrent: Arc<Torrent>,
}

impl TorrentSession {
    pub fn new() -> Self {
        // start tracker

        // start listener

        //start event loop

        todo!()
    }
    pub fn run(&self) {
        todo!()
    }
}

pub struct Client {
    cmd_tx: mpsc::UnboundedSender<ClientCommand>,
}

enum ClientCommand {
    AddTorrent(Torrent), // from TorrentParser
                         // Shutdown,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
        tokio::task::spawn(async move {
            // start disk actor

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    ClientCommand::AddTorrent(torrent) => {
                        let torrent_session = TorrentSession::new();

                        info!("recv TORRENT={:#?}", torrent);
                        // tokio::spawn(torrent_session.run()); // torrent logic (peers, pieces, etc.)
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
