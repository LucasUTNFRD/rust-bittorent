use std::net::SocketAddr;
use std::sync::Arc;

use crate::{
    bitfield::BitField,
    metainfo::{Info, TorrentInfo},
    piece_picker::PiecePicker,
    types::PieceHash,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // Helper function to create a mock TorrentInfo for testing
    fn create_mock_torrent(piece_count: usize, piece_length: i64) -> Arc<TorrentInfo> {
        let pieces = vec![PieceHash([0; 20]); piece_count];

        let info = Info {
            length: piece_count as i64 * piece_length,
            name: "test.txt".to_string(),
            piece_length,
            pieces,
        };

        Arc::new(TorrentInfo {
            announce: "http://tracker.example.com/announce".to_string(),
            announce_list: None,
            info,
            info_hash: crate::types::InfoHash([0; 20]),
        })
    }

    // Helper function to create a BitField with specific pieces set
    fn create_bitfield(size: usize, pieces: &[usize]) -> BitField {
        let mut bf = BitField::new(size);
        for &piece in pieces {
            bf.set_piece(piece);
        }
        bf
    }

    // Helper function to create a socket address
    fn socket_addr(ip: &str, port: u16) -> SocketAddr {
        SocketAddr::from_str(&format!("{}:{}", ip, port)).unwrap()
    }

    #[test]
    fn test_register_peer_and_check_interest() {
        let torrent = create_mock_torrent(10, 16384);
        let mut picker = PiecePicker::from(torrent);

        let peer1 = socket_addr("192.168.1.1", 8080);
        let peer2 = socket_addr("192.168.1.2", 8080);

        // Register peer1 with pieces 0, 2, 4
        let bf1 = create_bitfield(10, &[0, 2, 4]);
        picker.register_peer(peer1, bf1);

        // We should be interested in peer1
        assert!(
            picker.check_interest(peer1),
            "Should be interested in peer with pieces we don't have"
        );

        // Register peer2 with pieces 0, 3, 5
        let bf2 = create_bitfield(10, &[0, 3, 5]);
        picker.register_peer(peer2, bf2);

        // We should be interested in peer2
        assert!(
            picker.check_interest(peer2),
            "Should be interested in peer with pieces we don't have"
        );

        // Mark piece 0 as downloaded
        picker.mark_piece_downloaded(0);

        // We should still be interested in both peers
        assert!(
            picker.check_interest(peer1),
            "Should still be interested in peer1 (has pieces 2, 4)"
        );
        assert!(
            picker.check_interest(peer2),
            "Should still be interested in peer2 (has pieces 3, 5)"
        );

        // Mark all pieces from peer1 as downloaded
        picker.mark_piece_downloaded(2);
        picker.mark_piece_downloaded(4);

        // We should no longer be interested in peer1, but still in peer2
        assert!(
            !picker.check_interest(peer1),
            "Should not be interested in peer1 when we have all their pieces"
        );
        assert!(
            picker.check_interest(peer2),
            "Should still be interested in peer2 (has pieces 3, 5)"
        );
    }

    #[test]
    fn test_unregister_peer() {
        let torrent = create_mock_torrent(10, 16384);
        let mut picker = PiecePicker::from(torrent);

        let peer1 = socket_addr("192.168.1.1", 8080);
        let peer2 = socket_addr("192.168.1.2", 8080);

        // Register peer1 with pieces 0, 2, 4
        let bf1 = create_bitfield(10, &[0, 2, 4]);
        picker.register_peer(peer1, bf1);

        // Register peer2 with pieces 0, 3, 5
        let bf2 = create_bitfield(10, &[0, 3, 5]);
        picker.register_peer(peer2, bf2);

        // Unregister peer1
        picker.unregister_peer(&peer1);

        // We should still be interested in peer2
        assert!(
            picker.check_interest(peer2),
            "Should still be interested in peer2 after unregistering peer1"
        );

        // We should not be interested in peer1 (it's unregistered)
        assert!(
            !picker.check_interest(peer1),
            "Should not be interested in unregistered peer"
        );
    }

    #[test]
    fn test_update_peer() {
        let torrent = create_mock_torrent(10, 16384);
        let mut picker = PiecePicker::from(torrent);

        let peer = socket_addr("192.168.1.1", 8080);

        // Peer initially has pieces 0, 2
        let bf = create_bitfield(10, &[0, 2]);
        picker.register_peer(peer, bf);

        // Peer gets piece 3
        picker.update_peer(&peer, 3);

        // We should be interested in peer (it has pieces we don't have)
        assert!(
            picker.check_interest(peer),
            "Should be interested in peer after update"
        );

        // Mark all peer's pieces as downloaded
        picker.mark_piece_downloaded(0);
        picker.mark_piece_downloaded(2);
        picker.mark_piece_downloaded(3);

        // We should no longer be interested in this peer
        assert!(
            !picker.check_interest(peer),
            "Should not be interested in peer when we have all their pieces"
        );
    }

    #[test]
    fn test_strategy_switch_by_downloading_pieces() {
        let torrent = create_mock_torrent(10, 16384);
        let mut picker = PiecePicker::from(torrent);

        let peer = socket_addr("192.168.1.1", 8080);

        // Register peer with all pieces
        let bf = create_bitfield(10, &(0..10).collect::<Vec<_>>());
        picker.register_peer(peer, bf);

        // Get pieces before strategy switch (should be random first)
        let initial_requests = (0..10)
            .map(|_| {
                let blocks = picker.pick_piece(&peer).unwrap();
                let piece_idx = blocks[0].index as usize;
                picker.mark_piece_downloaded(piece_idx);
                piece_idx
            })
            .collect::<Vec<_>>();

        // With random first, pieces should not be in order
        let is_sequential = initial_requests.windows(2).all(|w| w[0] + 1 == w[1]);
        assert!(
            !is_sequential,
            "RandomFirst strategy should not pick pieces sequentially"
        );
    }

    #[test]
    fn test_pick_piece_rarest_first_behavior() {
        let torrent = create_mock_torrent(10, 16384);
        let mut picker = PiecePicker::from(torrent);

        // Set up peers with different piece availability
        let peer1 = socket_addr("192.168.1.1", 8080);
        let peer2 = socket_addr("192.168.1.2", 8080);
        let peer3 = socket_addr("192.168.1.3", 8080);

        // Force switch to RarestFirst strategy by marking 4 pieces as downloaded
        for i in 0..4 {
            picker.mark_piece_downloaded(i);
        }

        // Peer1 has pieces 4, 5
        picker.register_peer(peer1, create_bitfield(10, &[4, 5]));

        // Peer2 has pieces 4, 6, 7
        picker.register_peer(peer2, create_bitfield(10, &[4, 6, 7]));

        // Peer3 has pieces 4, 5, 6, 7, 8, 9
        picker.register_peer(peer3, create_bitfield(10, &[4, 5, 6, 7, 8, 9]));

        // Piece availability:
        // 4: All three peers (3)
        // 5: Peer1 and Peer3 (2)
        // 6: Peer2 and Peer3 (2)
        // 7: Peer2 and Peer3 (2)
        // 8: Only Peer3 (1) - rarest
        // 9: Only Peer3 (1) - rarest

        // Get piece from peer3 - should be one of the rarest: 8 or 9
        let blocks = picker.pick_piece(&peer3).unwrap();
        let piece_idx = blocks[0].index as usize;

        assert!(
            piece_idx == 8 || piece_idx == 9,
            "With RarestFirst strategy, peer3 should pick one of the rarest pieces (8 or 9), got {}",
            piece_idx
        );
    }

    #[test]
    fn test_get_blocks_and_piece_size() {
        let torrent = create_mock_torrent(2, 32768); // 32KB pieces
        let mut picker = PiecePicker::from(torrent);

        let peer = socket_addr("192.168.1.1", 8080);
        picker.register_peer(peer, create_bitfield(2, &[0, 1]));

        // Pick a piece and get its blocks
        let blocks = picker.pick_piece(&peer).unwrap();

        // The piece size is 32KB, and block size is 16KB, so we should get 2 blocks
        assert_eq!(blocks.len(), 2, "Should split a 32KB piece into 2 blocks");

        // Check first block
        assert_eq!(blocks[0].begin, 0, "First block should start at offset 0");
        assert_eq!(blocks[0].length, 16384, "Block size should be 16KB");

        // Check second block
        assert_eq!(
            blocks[1].begin, 16384,
            "Second block should start at offset 16KB"
        );
        assert_eq!(blocks[1].length, 16384, "Block size should be 16KB");
    }

    #[test]
    fn test_get_blocks_last_piece() {
        // Create a torrent with 2 pieces, the second one being smaller
        let torrent = Arc::new(TorrentInfo {
            announce: "http://tracker.example.com/announce".to_string(),
            announce_list: None,
            info: Info {
                length: 40000, // 40KB total
                name: "test.txt".to_string(),
                piece_length: 32768, // 32KB pieces
                pieces: vec![PieceHash([0; 20]); 2],
            },
            info_hash: crate::types::InfoHash([0; 20]),
        });

        let mut picker = PiecePicker::from(torrent);

        let peer = socket_addr("192.168.1.1", 8080);
        picker.register_peer(peer, create_bitfield(2, &[0, 1]));

        // Mark first piece as downloaded to ensure we get the second piece
        picker.mark_piece_downloaded(0);

        // Get blocks for the second piece (which is smaller)
        let blocks = picker.pick_piece(&peer).unwrap();

        // Check we get blocks for the right piece
        assert_eq!(blocks[0].index, 1, "Should get blocks for piece 1");

        // The last piece is 40000 - 32768 = 7232 bytes
        // We should get 1 block
        assert_eq!(blocks.len(), 1, "Last piece should have 1 block");
        assert_eq!(
            blocks[0].length, 7232,
            "Last block should have correct size"
        );
    }

    #[test]
    fn test_all_pieces_downloaded() {
        let torrent = create_mock_torrent(3, 16384);
        let mut picker = PiecePicker::from(torrent);

        assert!(
            !picker.all_pieces_downloaded(),
            "Initially, not all pieces should be downloaded"
        );

        // Mark pieces as downloaded one by one
        picker.mark_piece_downloaded(0);
        assert!(
            !picker.all_pieces_downloaded(),
            "After downloading 1/3 pieces, not all pieces should be downloaded"
        );

        picker.mark_piece_downloaded(1);
        assert!(
            !picker.all_pieces_downloaded(),
            "After downloading 2/3 pieces, not all pieces should be downloaded"
        );

        picker.mark_piece_downloaded(2);
        assert!(
            picker.all_pieces_downloaded(),
            "After downloading all pieces, all_pieces_downloaded should be true"
        );
    }

    #[test]
    fn test_piece_request_status() {
        let torrent = create_mock_torrent(5, 16384);
        let mut picker = PiecePicker::from(torrent);

        let peer1 = socket_addr("192.168.1.1", 8080);
        let peer2 = socket_addr("192.168.1.2", 8080);

        // Both peers have all pieces
        picker.register_peer(peer1, create_bitfield(5, &[0, 1, 2, 3, 4]));
        picker.register_peer(peer2, create_bitfield(5, &[0, 1, 2, 3, 4]));

        // Request a piece from peer1
        let blocks1 = picker.pick_piece(&peer1).unwrap();
        let piece_idx1 = blocks1[0].index as usize;

        // Request the same piece from peer2 should not be possible
        // The piece should be marked as Requested and not given out again
        let blocks2 = picker.pick_piece(&peer2).unwrap();
        let piece_idx2 = blocks2[0].index as usize;

        assert_ne!(
            piece_idx1, piece_idx2,
            "Same piece should not be picked twice"
        );

        // Mark piece as downloaded
        picker.mark_piece_downloaded(piece_idx1);

        // Now if we try to pick that piece again, we shouldn't get it
        for _ in 0..3 {
            let blocks = picker.pick_piece(&peer1).unwrap();
            let piece_idx = blocks[0].index as usize;
            assert_ne!(
                piece_idx, piece_idx1,
                "Downloaded piece should not be picked again"
            );
        }
    }
}
