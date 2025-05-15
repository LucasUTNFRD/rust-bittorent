use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{metainfo::Torrent, torrent_session::TorrentEvent, types::PeerId};

pub struct TrackerClient {}

impl TrackerClient {
    pub async fn start(
        torrent: Arc<Torrent>,
        client_id: PeerId,
        even_tx: mpsc::Sender<TorrentEvent>,
    ) {
        todo!()
    }
}
