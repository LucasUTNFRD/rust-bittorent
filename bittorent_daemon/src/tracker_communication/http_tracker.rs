use std::{net::SocketAddrV4, time::Duration};

use bittorrent_core::{bencode::Bencode, types::InfoHash};

use super::tracker_client::{DEFAULT_PORT, TrackerError};

#[derive(Debug)]
pub struct TrackerRequest {
    info_hash: InfoHash,
    peer_id: [u8; 20],
    port: u16,
    uploaded: u64,
    downloaded: u64,
    left: u64,
    compact: bool,
    event: Event,
}

#[derive(Debug)]
pub enum Event {
    Empty,
    Started,
    Completed,
    Stopped,
}

impl TrackerRequest {
    pub fn new(info_hash: InfoHash, peer_id: [u8; 20], left: u64, event: Event) -> Self {
        Self {
            info_hash,
            peer_id,
            left,
            event,
            compact: false,
            port: DEFAULT_PORT,
            downloaded: 0,
            uploaded: 0,
        }
    }

    pub fn build_url(&self, announce_url_str: &str) -> Result<reqwest::Url, TrackerError> {
        let mut url = reqwest::Url::parse(announce_url_str).unwrap();

        // Use urlencoding::encode_binary on the raw bytes
        let info_hash_encoded = urlencoding::encode_binary(&self.info_hash.0); // Access inner bytes
        // If peer_id remains [u8; 20] in TrackerRequest:
        let peer_id_encoded = urlencoding::encode_binary(&self.peer_id);
        // If you change peer_id to PeerId type in TrackerRequest:
        // let peer_id_encoded = urlencoding::encode_binary(&self.peer_id.0);

        let mut query_string = format!(
            "info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}",
            info_hash_encoded, // Use the encoded string
            peer_id_encoded,   // Use the encoded string
            self.port,
            self.uploaded,
            self.downloaded,
            self.left,
            if self.compact { "1" } else { "0" }
        );

        let event = match self.event {
            Event::Started => Some("started"),
            Event::Completed => Some("completed"),
            Event::Stopped => Some("stopped"),
            Event::Empty => None,
        };

        if let Some(event_str) = event {
            query_string.push_str(&format!("&event={}", event_str));
        };

        // Set the correctly formed query string
        url.set_query(Some(&query_string));

        Ok(url)
    }
}

#[derive(Debug, Clone)]
pub struct TrackerResponse {
    pub interval: Duration,
    pub peers: Vec<SocketAddrV4>,
    pub seeders: u64,
    pub leechers: u64,
}

// Define constants for Bencode dictionary keys for type safety and clarity
const FAILURE_REASON_KEY: &[u8] = b"failure reason";
const INTERVAL_KEY: &[u8] = b"interval";
const PEERS_KEY: &[u8] = b"peers";
const COMPLETE_KEY: &[u8] = b"complete";
const INCOMPLETE_KEY: &[u8] = b"incomplete";
// Constants for peer dictionary keys (if parsing non-compact peer list)
// const PEER_ID_KEY: &[u8] = b"peer id";
// const IP_KEY: &[u8] = b"ip";
// const PORT_KEY: &[u8] = b"port";

impl TrackerResponse {
    // Parses the response from a decoded Bencode value
    pub fn from_bencode(bencode_response: &Bencode) -> Result<Self, TrackerError> {
        // Ensure the top level is a dictionary
        let response_dict = match bencode_response {
            Bencode::Dict(dict) => dict,
            _ => {
                return Err(TrackerError::InvalidResponse(
                    "Response is not a dictionary".into(),
                ));
            }
        };

        // Check for failure reason first
        if let Some(reason_val) = response_dict.get(FAILURE_REASON_KEY) {
            if let Bencode::Bytes(reason_bytes) = reason_val {
                // Attempt to convert bytes to String, handle potential UTF-8 error
                let reason_str = String::from_utf8(reason_bytes.clone()).map_err(|e| {
                    TrackerError::InvalidResponse(format!(
                        "Failure reason is not valid UTF-8: {}",
                        e
                    ))
                })?;
                return Err(TrackerError::TrackerFailure(reason_str));
            } else {
                // If failure reason exists but isn't bytes, consider it invalid
                return Err(TrackerError::InvalidResponse(
                    "Failure reason is not a byte string".into(),
                ));
            }
        }

        // --- Extract required fields ---

        // Interval
        let interval_val = response_dict
            .get(INTERVAL_KEY)
            .ok_or_else(|| TrackerError::InvalidResponse("Missing 'interval' key".into()))?;
        let interval_secs = match interval_val {
            Bencode::Int(i) => {
                // Ensure interval is non-negative before converting
                if *i < 0 {
                    return Err(TrackerError::InvalidResponse(
                        "Interval cannot be negative".into(),
                    ));
                }
                *i as u64 // Convert to u64
            }
            _ => {
                return Err(TrackerError::InvalidResponse(
                    "Interval is not an integer".into(),
                ));
            }
        };

        // Peers
        let peers_val = response_dict
            .get(PEERS_KEY)
            .ok_or_else(|| TrackerError::InvalidResponse("Missing 'peers' key".into()))?;
        // let peers = Self::parse_peers(peers_val)?;

        let peers = match peers_val {
            Bencode::Bytes(bytes) => TrackerResponse::parse_peers(bytes),
            _ => return Err(TrackerError::InvalidPeerData),
        };

        // --- Extract optional fields ---

        // Complete (Seeders)
        let seeders = match response_dict.get(COMPLETE_KEY) {
            Some(Bencode::Int(i)) if *i >= 0 => *i as u64,
            Some(_) => {
                return Err(TrackerError::InvalidResponse(
                    "'complete' value is not a non-negative integer".into(),
                ));
            }
            None => 0, // Default to 0 if key is missing
        };

        // Incomplete (Leechers)
        let leechers = match response_dict.get(INCOMPLETE_KEY) {
            Some(Bencode::Int(i)) if *i >= 0 => *i as u64,
            Some(_) => {
                return Err(TrackerError::InvalidResponse(
                    "'incomplete' value is not a non-negative integer".into(),
                ));
            }
            None => 0, // Default to 0 if key is missing
        };

        Ok(TrackerResponse {
            interval: Duration::from_secs(interval_secs),
            peers,
            seeders,
            leechers,
        })
    }

    pub fn parse_peers(bytes: &[u8]) -> Vec<SocketAddrV4> {
        let mut peers = Vec::new();
        for chunk in bytes.chunks(6) {
            if chunk.len() != 6 {
                continue; // Skip incomplete chunks
            }
            let ip = SocketAddrV4::new(
                std::net::Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]),
                u16::from_be_bytes([chunk[4], chunk[5]]),
            );
            peers.push(ip);
        }
        peers
    }
}
