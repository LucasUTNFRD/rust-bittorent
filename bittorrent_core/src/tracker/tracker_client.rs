use std::{net::SocketAddr, sync::Arc, time::Duration};

use rand::seq::SliceRandom;
use reqwest::{Client as HttpClient, StatusCode, Url};
use thiserror::Error;
use tokio::{sync::mpsc, task::JoinHandle, time};
use tracing::{debug, error, info, warn};

use crate::{
    bencode::{Bencode, BencodeError},
    metainfo::TorrentInfo,
    torrent_session::TorrentMessage,
    types::PeerId,
};

const DEFAULT_PORT: u16 = 6881;
const DEFAULT_ANNOUNCE_INTERVAL: u32 = 120; // 2 minutes
const DEFAULT_PEER_COUNT: u32 = 50;

/// Represents a peer from the tracker response
#[derive(Debug, Clone)]
pub struct Peer {
    pub ip: String,
    pub port: u16,
    pub peer_id: Option<String>,
}

impl Peer {
    pub fn socket_addr(&self) -> Option<SocketAddr> {
        self.ip
            .parse()
            .ok()
            .map(|ip| SocketAddr::new(ip, self.port))
    }
}

#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("HTTP request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Failed to parse tracker URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Bencode error: {0}")]
    BencodeError(#[from] BencodeError),

    #[error("Tracker returned error: {0}")]
    TrackerError(String),

    #[error("Missing field in tracker response: {0}")]
    MissingField(String),

    #[error("Invalid tracker response format: {0}")]
    InvalidResponseFormat(String),

    #[error("No working trackers available")]
    NoWorkingTrackers,
}

/// Represents an announce event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnounceEvent {
    Started,
    Stopped,
    Completed,
    None,
}

impl AnnounceEvent {
    pub fn as_str(&self) -> Option<&'static str> {
        match self {
            AnnounceEvent::Started => Some("started"),
            AnnounceEvent::Stopped => Some("stopped"),
            AnnounceEvent::Completed => Some("completed"),
            AnnounceEvent::None => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackerStatus {
    pub interval: u32,
    pub min_interval: Option<u32>,
    pub tracker_id: Option<String>,
    pub complete: u32,
    pub incomplete: u32,
    pub peers: Vec<Peer>,
}

/// Tracks the state of each tracker in the tier list
#[derive(Debug, Clone)]
struct TrackerTier {
    trackers: Vec<String>,
}

pub struct TrackerClient {
    torrent: Arc<TorrentInfo>,
    client_id: PeerId,
    torrent_tx: mpsc::Sender<TorrentMessage>,
    http_client: HttpClient,
    active_task: Option<JoinHandle<()>>,
    tiers: Vec<TrackerTier>,
    pub current_tracker_url: Option<String>,
    tracker_id: Option<String>,
    port: u16,
    upload_total: u64,
    download_total: u64,
}

impl TrackerClient {
    pub fn new(
        torrent: Arc<TorrentInfo>,
        client_id: PeerId,
        torrent_tx: mpsc::Sender<TorrentMessage>,
    ) -> Self {
        let mut tiers = Vec::new();
        // Initialize tracker tiers based on BEP-0012
        if let Some(announce_list) = &torrent.announce_list {
            for tier in announce_list {
                // Create a shuffled copy of the tier
                let mut shuffled_tier = tier.clone();
                shuffled_tier.shuffle(&mut rand::thread_rng());
                tiers.push(TrackerTier {
                    trackers: shuffled_tier,
                });
            }
        } else {
            // Single tracker URL
            tiers.push(TrackerTier {
                trackers: vec![torrent.announce.clone()],
            });
        }

        Self {
            torrent,
            client_id,
            torrent_tx,
            http_client: HttpClient::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| HttpClient::new()),
            active_task: None,
            tiers,
            current_tracker_url: None,
            tracker_id: None,
            port: DEFAULT_PORT,
            upload_total: 0,
            download_total: 0,
        }
    }

    pub async fn start(&mut self) {
        // Cancel any existing task
        if let Some(task) = self.active_task.take() {
            task.abort();
        }

        // Clone everything needed for the task
        let torrent = self.torrent.clone();
        let client_id = self.client_id;
        let torrent_tx = self.torrent_tx.clone();
        let http_client = self.http_client.clone();
        let tiers = self.tiers.clone();
        let tracker_id = self.tracker_id.clone();
        let port = self.port;

        // Start a task for the announce loop
        let task = tokio::spawn(async move {
            info!("Starting tracker announce loop");

            // Create a new client instance just for this task
            // This is different from self, but it's a separate instance that won't cause a loop
            let mut tracker = TrackerClient {
                torrent,
                client_id,
                torrent_tx,
                http_client,
                active_task: None, // Important: no nested active_task
                tiers,
                current_tracker_url: None,
                tracker_id,
                port,
                upload_total: 0,
                download_total: 0,
            };

            // Initial announce with "started" event
            info!("Performing initial tracker announce with 'started' event");
            let initial_status = match tracker.announce_to_tracker(AnnounceEvent::Started).await {
                Ok(status) => {
                    info!(
                        "Initial tracker announce successful, interval: {} seconds",
                        status.interval
                    );
                    status
                }
                Err(e) => {
                    error!("Failed to announce start to tracker: {}", e);
                    return;
                }
            };

            // Send initial peer list to torrent session
            if !initial_status.peers.is_empty() {
                let sockets = initial_status
                    .peers
                    .iter()
                    .filter_map(|p| p.socket_addr())
                    .collect();

                if let Err(e) = tracker
                    .torrent_tx
                    .send(TorrentMessage::PeerList(sockets))
                    .await
                {
                    error!("Failed to send initial peers to torrent session: {:?}", e);
                    return;
                }
            }

            // Regular announce loop with the interval from the tracker
            let mut interval = time::interval(Duration::from_secs(initial_status.interval as u64));
            interval.tick().await;
            loop {
                interval.tick().await;
                info!("Performing regular tracker announce");

                match tracker.announce_to_tracker(AnnounceEvent::None).await {
                    Ok(status) => {
                        info!(
                            "Tracker announce successful, next interval: {} seconds",
                            status.interval
                        );
                        // Update interval for next announce
                        interval = time::interval(Duration::from_secs(status.interval as u64));

                        // Send peer list to torrent session
                        if !status.peers.is_empty() {
                            let sockets = status
                                .peers
                                .iter()
                                .filter_map(|p| p.socket_addr())
                                .collect();

                            debug!("Sending {} peers to torrent session", status.peers.len());
                            if let Err(e) = tracker
                                .torrent_tx
                                .send(TorrentMessage::PeerList(sockets))
                                .await
                            {
                                error!("Failed to send peers to torrent session: {:?}", e);
                                return; // Exit loop if we can't send anymore
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to announce to tracker: {}", e);
                        // Continue using the current interval - will try again next time
                    }
                }
            }
        });

        self.active_task = Some(task);
    }

    pub async fn stop(&mut self) {
        // Send stopped event
        if let Err(e) = self.announce_to_tracker(AnnounceEvent::Stopped).await {
            warn!("Failed to announce stop to tracker: {}", e);
        }

        // Cancel the announce loop
        if let Some(task) = self.active_task.take() {
            task.abort();
        }
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    pub fn update_stats(&mut self, uploaded: u64, downloaded: u64) {
        self.upload_total = uploaded;
        self.download_total = downloaded;
    }

    pub async fn announce_to_tracker(
        &mut self,
        event: AnnounceEvent,
    ) -> Result<TrackerStatus, TrackerError> {
        // Try each tier and each tracker in order
        let mut final_error = None;

        for tier_index in 0..self.tiers.len() {
            // Try each tracker in this tier
            let mut tracker_index = 0;
            let tier_len = self.tiers[tier_index].trackers.len();

            while tracker_index < tier_len {
                let tracker_url = self.tiers[tier_index].trackers[tracker_index].clone();
                match self.announce_to_url(&tracker_url, event).await {
                    Ok(status) => {
                        // Move successful tracker to the front of its tier
                        if tracker_index > 0 {
                            // Need to clone the URL first and then move it
                            self.tiers[tier_index].trackers.remove(tracker_index);
                            self.tiers[tier_index]
                                .trackers
                                .insert(0, tracker_url.clone());
                        }

                        self.current_tracker_url = Some(tracker_url);
                        return Ok(status);
                    }
                    Err(e) => {
                        warn!("Failed to announce to tracker {}: {}", tracker_url, e);
                        final_error = Some(e);
                        tracker_index += 1;
                    }
                }
            }
        }

        // If we get here, all trackers failed
        match final_error {
            Some(e) => Err(e),
            None => Err(TrackerError::NoWorkingTrackers),
        }
    }

    async fn announce_to_url(
        &mut self,
        tracker_url: &str,
        event: AnnounceEvent,
    ) -> Result<TrackerStatus, TrackerError> {
        let mut url = Url::parse(tracker_url)?;

        // Calculate bytes left
        let bytes_left = self.torrent.info.length as u64 - self.download_total;

        // Prepare query parameters
        let mut query_pairs = vec![
            ("info_hash", self.torrent.info_hash.0.to_vec()),
            ("peer_id", self.client_id.0.to_vec()),
            ("port", self.port.to_string().into_bytes()),
            ("uploaded", self.upload_total.to_string().into_bytes()),
            ("downloaded", self.download_total.to_string().into_bytes()),
            ("left", bytes_left.to_string().into_bytes()),
            ("compact", "1".into()), // Request compact response
            ("numwant", DEFAULT_PEER_COUNT.to_string().into_bytes()),
        ];

        // Add optional event parameter
        if let Some(event_str) = event.as_str() {
            query_pairs.push(("event", event_str.into()));
        }

        // Add tracker ID if we have one
        if let Some(ref id) = self.tracker_id {
            query_pairs.push(("trackerid", id.clone().into_bytes()));
        }

        // Build the query string
        let mut query_string = String::new();

        for (key, value) in &query_pairs {
            if !query_string.is_empty() {
                query_string.push('&');
            }

            query_string.push_str(key);
            query_string.push('=');

            if *key == "info_hash" || *key == "peer_id" {
                // Binary data needs special URL encoding
                for &byte in value {
                    // URL encode each byte
                    query_string.push('%');
                    query_string.push_str(&format!("{:02X}", byte));
                }
            } else {
                // For other parameters, use standard encoding
                query_string
                    .push_str(&url::form_urlencoded::byte_serialize(value).collect::<String>());
            }
        }

        // Apply the encoded query string
        url.set_query(Some(&query_string));

        debug!("Announcing to tracker: {}", url);

        // Send the HTTP request
        let response = self.http_client.get(url).send().await?;

        let status = response.status();
        if status != StatusCode::OK {
            return Err(TrackerError::InvalidResponseFormat(format!(
                "HTTP status: {}",
                status
            )));
        }

        // Get and parse the response bytes
        let bytes = response.bytes().await?;
        let bencode = Bencode::decode(&bytes)?;

        // Check for error from tracker
        if let Some(Bencode::Bytes(failure_reason)) = bencode.get(b"failure reason") {
            let reason = String::from_utf8_lossy(failure_reason).to_string();
            return Err(TrackerError::TrackerError(reason));
        }

        if let Some(Bencode::Bytes(warning)) = bencode.get(b"warning message") {
            let warning_msg = String::from_utf8_lossy(warning).to_string();
            warn!("Tracker warning: {}", warning_msg);
        }

        // Extract interval
        let interval = match bencode.get(b"interval") {
            Some(Bencode::Int(i)) if *i > 0 => *i as u32,
            _ => DEFAULT_ANNOUNCE_INTERVAL,
        };

        // Extract min interval (optional)
        let min_interval = match bencode.get(b"min interval") {
            Some(Bencode::Int(i)) if *i > 0 => Some(*i as u32),
            _ => None,
        };

        // Extract tracker id (optional)
        let tracker_id = match bencode.get(b"tracker id") {
            Some(Bencode::Bytes(id)) => {
                let id_str = String::from_utf8_lossy(id).to_string();
                self.tracker_id = Some(id_str.clone()); // Store for future announces
                Some(id_str)
            }
            _ => None,
        };

        // Extract complete and incomplete peers
        let complete = match bencode.get(b"complete") {
            Some(Bencode::Int(i)) => *i as u32,
            _ => 0,
        };

        let incomplete = match bencode.get(b"incomplete") {
            Some(Bencode::Int(i)) => *i as u32,
            _ => 0,
        };

        // Parse peers - could be dictionary model or binary model
        let peers = self.parse_peers(&bencode)?;

        Ok(TrackerStatus {
            interval,
            min_interval,
            tracker_id,
            complete,
            incomplete,
            peers,
        })
    }

    pub(crate) fn parse_peers(&self, response: &Bencode) -> Result<Vec<Peer>, TrackerError> {
        match response.get(b"peers") {
            Some(Bencode::Bytes(bytes)) => {
                // Compact format: 6 bytes per peer (4 for IP, 2 for port)
                if bytes.len() % 6 != 0 {
                    return Err(TrackerError::InvalidResponseFormat(
                        "Invalid compact peers format".to_string(),
                    ));
                }

                let mut peers = Vec::with_capacity(bytes.len() / 6);

                for chunk in bytes.chunks_exact(6) {
                    // IP address: first 4 bytes
                    let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);

                    // Port: next 2 bytes in network byte order (big-endian)
                    let port = ((chunk[4] as u16) << 8) | (chunk[5] as u16);

                    peers.push(Peer {
                        ip,
                        port,
                        peer_id: None, // Not available in compact format
                    });
                }

                Ok(peers)
            }
            Some(Bencode::List(peer_list)) => {
                // Dictionary model
                let mut peers = Vec::with_capacity(peer_list.len());

                for peer_item in peer_list {
                    if let Bencode::Dict(peer_dict) = peer_item {
                        let ip = match peer_dict.get(b"ip".to_vec().as_slice()) {
                            Some(Bencode::Bytes(bytes)) => {
                                String::from_utf8_lossy(bytes).to_string()
                            }
                            _ => return Err(TrackerError::MissingField("ip".to_string())),
                        };

                        let port = match peer_dict.get(b"port".to_vec().as_slice()) {
                            Some(Bencode::Int(p)) => *p as u16,
                            _ => return Err(TrackerError::MissingField("port".to_string())),
                        };

                        let peer_id = match peer_dict.get(b"peer id".to_vec().as_slice()) {
                            Some(Bencode::Bytes(id)) => {
                                Some(String::from_utf8_lossy(id).to_string())
                            }
                            _ => None,
                        };

                        peers.push(Peer { ip, port, peer_id });
                    }
                }

                Ok(peers)
            }
            _ => Err(TrackerError::MissingField("peers".to_string())),
        }
    }
}
