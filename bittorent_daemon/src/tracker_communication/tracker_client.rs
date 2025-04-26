use bittorrent_core::{
    bencode::{Bencode, BencodeError},
    types::{InfoHash, PeerId},
};
use thiserror::Error;
use tokio::sync::mpsc::{self, Sender};

pub struct TrackerClient {
    pub announce_url: String,
    pub info_hash: InfoHash,
    pub peer_id: PeerId,
    pub port: u16,

    //Tracking of torrrent
    uploaded: u64,
    downloaded: u64,
    left: u64,

    command_rx: mpsc::Receiver<TrackerMessage>,
    command_tx: mpsc::Sender<TrackerMessage>,
}

#[derive(Debug)]
pub enum TrackerMessage {
    Announce {
        event: Event,
        info_hash: [u8; 20],
        downloaded: u64,
        uploaded: u64,
        left: u64,
    },
}

pub struct TrackerContext {}

#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("HTTP request failed: {0}")]
    HttpRequest(#[from] reqwest::Error),
    #[error("Reading bytes: {0}")]
    Bytes(reqwest::Error),
    #[error("Bencode decoding failed:{0}")]
    Bencode(BencodeError),
    #[error("Tracker returned failure: {0}")]
    TrackerFailure(String),
    #[error("Invalid response data: {0}")]
    InvalidResponse(String),
    #[error("Invalid peer data received")]
    InvalidPeerData,
}

use rand::Rng;
use tracing::{debug, field::debug, info};

pub const DEFAULT_PORT: u16 = 6881;

use super::http_tracker::{Event, TrackerRequest, TrackerResponse};

fn generate_peer_id() -> PeerId {
    let mut peer_id = [0u8; 20];
    peer_id[0..3].copy_from_slice(b"-RS"); // Client identifier
    rand::rng().fill(&mut peer_id[3..]); // Random bytes
    PeerId(peer_id)
}

// --- Tracker Actor Implementation ---
impl TrackerClient {
    pub fn new(
        announce_url: String,
        info_hash: InfoHash,
        port: u16,
        torrent_len: u64,
        peer_id: PeerId,
    ) -> (Self, Sender<TrackerMessage>) {
        // let peer_id = generate_peer_id();
        let (tx, rx) = mpsc::channel(32);
        let client = TrackerClient {
            announce_url,
            info_hash,
            peer_id,
            uploaded: 0,
            downloaded: 0,
            left: torrent_len,
            port,
            command_tx: tx.clone(),
            command_rx: rx,
        };

        (client, tx)
    }

    pub async fn connect(&self) -> Result<TrackerResponse, TrackerError> {
        let request = TrackerRequest::new(self.info_hash, self.peer_id, self.left, Event::Started);

        let query_url = request.build_url(&self.announce_url)?;
        debug!("Query:{:?}", query_url);

        let resp = reqwest::get(query_url)
            .await
            .map_err(TrackerError::HttpRequest)?
            .bytes()
            .await
            .map_err(TrackerError::Bytes)?;

        let bencoded_resp = Bencode::decode(&resp).map_err(TrackerError::Bencode)?;
        let resp = TrackerResponse::from_bencode(&bencoded_resp)?;

        debug(&resp);

        Ok(resp)
    }

    // TODO: Implement this so the torrent communicates only via messages
    pub async fn run(&mut self) {
        todo!("Not implemented yet")
    }
}
