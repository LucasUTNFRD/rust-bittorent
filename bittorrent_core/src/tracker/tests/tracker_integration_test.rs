use std::{
    // net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
    sync::Arc,
};

use tokio::{sync::mpsc, time::timeout};

use crate::{
    torrent_parser::TorrentParser,
    torrent_session::TorrentMessage,
    tracker::tracker_client::{AnnounceEvent, TrackerClient},
    types::PeerId,
};

// Test specific constants
const TEST_PORT: u16 = 6881;
const TIMEOUT_SECONDS: u64 = 5;

// Create a test peer ID
fn create_test_peer_id() -> PeerId {
    let mut id = [0u8; 20];
    id[0..3].copy_from_slice(b"-TE");
    PeerId(id)
}

const EXAMPLE_TORRENT: &str = "sample.torrent";
const EXAXMPLE_TORRENT_MULTIPLE_ANNOUNCE: &str = "ubuntu-25.04-desktop-amd64.iso.torrent";

// Helper function to find the sample.torrent file
fn find_sample_torrent(filename: &str) -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR env var not set during test");
    let mut filepath = PathBuf::from(manifest_dir);

    // Navigate to the workspace root
    assert!(
        filepath.pop(),
        "Failed to navigate to workspace root from manifest dir"
    );

    // Path to the sample torrent file
    filepath.push("sample_torrents");
    filepath.push(filename);

    assert!(
        filepath.exists(),
        "Test torrent file not found at: {}",
        filepath.display()
    );

    filepath
}

// Test that we can communicate with the real tracker
#[tokio::test]
async fn test_real_tracker_announce() {
    let filepath = find_sample_torrent(EXAMPLE_TORRENT);
    let torrent = TorrentParser::parse(&filepath).expect("Failed to parse sample.torrent file");

    // Create a channel to receive messages from the tracker client
    let (tx, mut rx) = mpsc::channel(10);
    let peer_id = create_test_peer_id();

    // Create a tracker client with the parsed torrent
    let torrent_arc = Arc::new(torrent);
    let mut client = TrackerClient::new(torrent_arc.clone(), peer_id, tx);

    // Set the port to match our test configuration
    client.set_port(TEST_PORT);

    // Make a single announce call (don't start the announce loop)
    let result = timeout(
        std::time::Duration::from_secs(TIMEOUT_SECONDS),
        client.announce_to_tracker(AnnounceEvent::Started),
    )
    .await;

    // Verify that the call completed successfully
    assert!(result.is_ok(), "Tracker announce timed out");
    let tracker_status = result.unwrap().expect("Failed to get tracker response");

    // Verify that we received peers in the response
    assert!(
        !tracker_status.peers.is_empty(),
        "No peers received from tracker"
    );

    // Print the actual peers received from the tracker for debugging
    println!(
        "Received {} peers from tracker:",
        tracker_status.peers.len()
    );
    for peer in &tracker_status.peers {
        println!("Peer: {}:{}", peer.ip, peer.port);
    }

    // Verify we got the expected peer IPs (the ports might change)
    let expected_ips = vec!["165.232.38.164", "165.232.41.73", "165.232.35.114"];

    // Check that all expected IPs are present
    for expected_ip in &expected_ips {
        assert!(
            tracker_status.peers.iter().any(|p| &p.ip == expected_ip),
            "Expected peer with IP {} not found in tracker response",
            expected_ip
        );
    }

    // Check that we have exactly 3 peers (as the test tracker always returns 3)
    assert_eq!(
        tracker_status.peers.len(),
        3,
        "Expected exactly 3 peers from tracker"
    );

    // Alternative approach: Wait for the PeerList message from the client
    // This tests the integration with the torrent session
    client.start().await;

    // Wait for the PeerList message from the tracker client
    let peer_msg = timeout(std::time::Duration::from_secs(TIMEOUT_SECONDS), rx.recv()).await;

    assert!(peer_msg.is_ok(), "Timed out waiting for peer list message");

    if let Ok(Some(TorrentMessage::PeerList(peers))) = peer_msg {
        assert!(!peers.is_empty(), "Received empty peer list");

        // Print the actual peers received in the peer list message
        println!("Received {} peers in PeerList message:", peers.len());
        for peer in &peers {
            println!("Peer: {}:{}", peer.ip(), peer.port());
        }

        // Verify we got the same expected peer IPs in the message
        let expected_ips = vec!["165.232.38.164", "165.232.41.73", "165.232.35.114"];

        // Check that all expected IPs are present
        for expected_ip in &expected_ips {
            assert!(
                peers.iter().any(|p| &p.ip().to_string() == expected_ip),
                "Expected peer with IP {} not found in PeerList message",
                expected_ip
            );
        }

        // Check that we have exactly 3 peers
        assert_eq!(
            peers.len(),
            3,
            "Expected exactly 3 peers in PeerList message"
        );
    } else {
        panic!("Did not receive expected PeerList message");
    }

    // Stop the tracker client
    client.stop().await;
}

// Test that we can communicate with the real tracker
#[tokio::test]
async fn test_real_tracker_ipv6() {
    let filepath = find_sample_torrent(EXAXMPLE_TORRENT_MULTIPLE_ANNOUNCE);
    let torrent = TorrentParser::parse(&filepath).expect("Failed to parse sample.torrent file");

    // Create a channel to receive messages from the tracker client
    let (tx, mut rx) = mpsc::channel(10);
    let peer_id = create_test_peer_id();

    // Create a tracker client with the parsed torrent
    let torrent_arc = Arc::new(torrent);
    let mut client = TrackerClient::new(torrent_arc.clone(), peer_id, tx);

    // Set the port to match our test configuration
    client.set_port(TEST_PORT);

    // Make a single announce call (don't start the announce loop)
    let result = timeout(
        std::time::Duration::from_secs(TIMEOUT_SECONDS),
        client.announce_to_tracker(AnnounceEvent::Started),
    )
    .await;

    // Verify that the call completed successfully
    assert!(result.is_ok(), "Tracker announce timed out");
    let tracker_status = result.unwrap().expect("Failed to get tracker response");

    // Verify that we received peers in the response
    assert!(
        !tracker_status.peers.is_empty(),
        "No peers received from tracker"
    );

    // Print the actual peers received from the tracker for debugging
    println!(
        "Received {} peers from tracker:",
        tracker_status.peers.len()
    );
    for peer in &tracker_status.peers {
        println!("Peer: {}:{}", peer.ip, peer.port);
    }

    // Alternative approach: Wait for the PeerList message from the client
    // This tests the integration with the torrent session
    client.start().await;

    // Wait for the PeerList message from the tracker client
    let peer_msg = timeout(std::time::Duration::from_secs(TIMEOUT_SECONDS), rx.recv()).await;

    assert!(peer_msg.is_ok(), "Timed out waiting for peer list message");

    if let Ok(Some(TorrentMessage::PeerList(peers))) = peer_msg {
        assert!(!peers.is_empty(), "Received empty peer list");

        // Print the actual peers received in the peer list message
        println!("Received {} peers in PeerList message:", peers.len());
        for peer in &peers {
            println!("Peer: {}:{}", peer.ip(), peer.port());
        }
    } else {
        panic!("Did not receive expected PeerList message");
    }

    // Stop the tracker client
    client.stop().await;
}
