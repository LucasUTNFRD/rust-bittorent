use std::{net::SocketAddrV4, path::PathBuf, sync::Arc};

use bittorrent_core::metainfo::Torrent;
use thiserror::Error;
use tracing::{debug, info};

use crate::tracker_communication::tracker_client::{TrackerClient, TrackerError};

pub struct TorrentSession {
    torrent: Arc<Torrent>,
    save_dir: PathBuf,
    port: u16,
}

#[derive(Debug, Error)]
pub enum TorrentSessionError {
    #[error("Tracker Error: {0}")]
    Tracker(TrackerError),
}

impl TorrentSession {
    pub fn new(torrent: Arc<Torrent>, save_dir: PathBuf, port: u16) -> Self {
        Self {
            torrent,
            save_dir,
            port,
        }
    }

    pub async fn start_running_session(&mut self) -> Result<(), TorrentSessionError> {
        info!(
            "Started running a new torrent session {:?}",
            self.torrent.info.name
        );

        //1. Contact a a tracker
        let (tracker_client, client_tx) = TrackerClient::new(
            self.torrent.announce.clone(),
            self.torrent.info_hash,
            self.port,
            self.torrent.info.length as u64,
        );

        info!("Trying to cnnect with tracker");
        let resp = tracker_client
            .connect()
            .await
            .map_err(TorrentSessionError::Tracker)?;
        debug!("{:?}", &resp);

        //
        //2. handle inbound connections

        //
        //3. handle outbound connections
        info!("Here received a resp from tracker");
        self.handle_outbound_peer(resp.peers).await;
        Ok(())
    }

    /// Remote peer initiated the connection to our client
    /// Another peer in the swarm actively reached out to our client's listening port
    /// and established a TCP connection.
    async fn handle_inbound_peer() {}

    /// Our client initated the connection to the remote peer
    /// this usually happens when your client receives a list of peers from a tracker
    async fn handle_outbound_peer(&self, peers: Vec<SocketAddrV4>) {
        info!("We should connect to the following peers");
        for peer in peers.iter() {
            info!("Peer {:?}", peer);
        }
    }
}

// TODO:
// Objective 1: via cli add a torrent, the daemon starts  a new torrent session, then contacts
// tracker to get peers, just print peers
