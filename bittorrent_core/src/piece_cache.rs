use std::{collections::HashMap, sync::Arc};

use bytes::{Bytes, BytesMut};

use crate::{metainfo::TorrentInfo, piece_picker::Block};

pub struct PieceCache {
    piece_map: HashMap<usize, PieceMetadata>,
}

struct PieceMetadata {
    buffer: BytesMut,
    piece_length: usize,
    downloaded: usize,
}

impl From<Arc<TorrentInfo>> for PieceCache {
    fn from(value: Arc<TorrentInfo>) -> Self {
        let mut piece_map = HashMap::new();
        for i in 0..value.get_total_pieces() {
            let piece_length = value.get_piece_len(i) as usize;
            let mut piece_buffer = BytesMut::with_capacity(piece_length as usize);
            piece_buffer.resize(piece_length, 0);
            let piece_metadata = PieceMetadata {
                buffer: piece_buffer,
                piece_length,
                downloaded: 0,
            };
            piece_map.insert(i, piece_metadata);
        }
        Self { piece_map }
    }
}

impl PieceCache {
    /// Inserts a block to the piece cache.
    ///
    /// If after block insertion the piece obtained all the blocks needed, the entry is removed and
    /// this will yield the piecen index and the data asociated of that index.
    pub fn insert_block(&mut self, block: Block) -> Option<(usize, Bytes)> {
        let index = block.index as usize;

        let piece = self.piece_map.get_mut(&index)?;
        let offset = block.begin as usize;
        piece.buffer[offset..offset + block.data.len()].copy_from_slice(&block.data);
        piece.downloaded += block.data.len();

        if piece.downloaded == piece.piece_length {
            // Clone the buffer before removing the piece from the map
            let completed_piece = piece.buffer.clone().freeze();

            // Remove the piece from the cache
            self.piece_map.remove(&index);

            return Some((index, completed_piece));
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::PieceCache;
    use crate::{
        metainfo::{Info, TorrentInfo},
        piece_picker::Block,
        types::{InfoHash, PieceHash},
    };
    use bytes::BytesMut;
    use std::sync::Arc;

    fn create_mock_torrent_info() -> Arc<TorrentInfo> {
        // Create a simple torrent info with 3 pieces of 16KB each
        let piece_length = 16384; // 16KB
        let total_length = 3 * piece_length;

        // Create mock piece hashes (not actually used in the tests)
        let mock_hash = [0u8; 20];
        let pieces = vec![PieceHash(mock_hash); 3];

        let info = Info {
            length: total_length as i64,
            name: "test_file".to_string(),
            piece_length: piece_length as i64,
            pieces,
        };

        let mock_hash_array = [0u8; 20];

        Arc::new(TorrentInfo {
            announce: "http://example.com/announce".to_string(),
            announce_list: None,
            info,
            info_hash: InfoHash::from(mock_hash_array),
        })
    }

    #[test]
    fn insert_block() {
        let torrent_info = create_mock_torrent_info();
        let mut cache = PieceCache::from(torrent_info);

        // Create a block for the first piece
        let data = BytesMut::from(&[42u8; 8192][..]).freeze();
        let block = Block {
            index: 0,
            begin: 0,
            data,
        };

        // Insert the block
        let result = cache.insert_block(block);

        // The piece shouldn't be complete yet since we only inserted half of it
        assert!(result.is_none());

        // Verify that the data was inserted correctly
        let piece = &cache.piece_map.get(&0).unwrap();
        assert_eq!(piece.downloaded, 8192);
        for i in 0..8192 {
            assert_eq!(piece.buffer[i], 42);
        }
    }

    #[test]
    fn insert_block_yields_download_completed() {
        let torrent_info = create_mock_torrent_info();
        let mut cache = PieceCache::from(torrent_info);

        // Create and insert first half of the piece
        let data1 = BytesMut::from(&[1u8; 8192][..]).freeze();
        let block1 = Block {
            index: 0,
            begin: 0,
            data: data1,
        };
        let result1 = cache.insert_block(block1);
        assert!(result1.is_none());

        // Create and insert second half of the piece
        let data2 = BytesMut::from(&[2u8; 8192][..]).freeze();
        let block2 = Block {
            index: 0,
            begin: 8192,
            data: data2,
        };

        // This should complete the piece
        let result2 = cache.insert_block(block2);

        // Verify that the piece is reported as complete
        assert!(result2.is_some());
        let (index, data) = result2.unwrap();
        assert_eq!(index, 0);
        assert_eq!(data.len(), 16384);

        // Verify the content of the completed piece
        for i in 0..8192 {
            assert_eq!(data[i], 1);
        }
        for i in 8192..16384 {
            assert_eq!(data[i], 2);
        }
    }

    #[test]
    fn insert_block_different_pieces() {
        let torrent_info = create_mock_torrent_info();
        let mut cache = PieceCache::from(torrent_info);

        // Insert into piece 0
        let data0 = BytesMut::from(&[1u8; 4096][..]).freeze();
        let block0 = Block {
            index: 0,
            begin: 0,
            data: data0,
        };
        let result0 = cache.insert_block(block0);
        assert!(result0.is_none());

        // Insert into piece 1
        let data1 = BytesMut::from(&[2u8; 4096][..]).freeze();
        let block1 = Block {
            index: 1,
            begin: 0,
            data: data1,
        };
        let result1 = cache.insert_block(block1);
        assert!(result1.is_none());

        // Verify that both pieces have the correct data
        let piece0 = &cache.piece_map.get(&0).unwrap();
        assert_eq!(piece0.downloaded, 4096);
        for i in 0..4096 {
            assert_eq!(piece0.buffer[i], 1);
        }

        let piece1 = &cache.piece_map.get(&1).unwrap();
        assert_eq!(piece1.downloaded, 4096);
        for i in 0..4096 {
            assert_eq!(piece1.buffer[i], 2);
        }
    }

    #[test]
    fn test_piece_removed_after_completion() {
        let torrent_info = create_mock_torrent_info();
        let mut cache = PieceCache::from(torrent_info);

        // Check that we initially have all 3 pieces in the cache
        assert_eq!(cache.piece_map.len(), 3);
        assert!(cache.piece_map.contains_key(&0));
        assert!(cache.piece_map.contains_key(&1));
        assert!(cache.piece_map.contains_key(&2));

        // Complete piece 0
        let data = BytesMut::from(&[1u8; 16384][..]).freeze();
        let block = Block {
            index: 0,
            begin: 0,
            data,
        };

        let result = cache.insert_block(block);
        assert!(result.is_some());

        // Verify piece 0 is removed from cache
        assert_eq!(cache.piece_map.len(), 2);
        assert!(!cache.piece_map.contains_key(&0));
        assert!(cache.piece_map.contains_key(&1));
        assert!(cache.piece_map.contains_key(&2));

        // Complete piece 1
        let data = BytesMut::from(&[2u8; 16384][..]).freeze();
        let block = Block {
            index: 1,
            begin: 0,
            data,
        };

        let result = cache.insert_block(block);
        assert!(result.is_some());

        // Verify piece 1 is also removed from cache
        assert_eq!(cache.piece_map.len(), 1);
        assert!(!cache.piece_map.contains_key(&0));
        assert!(!cache.piece_map.contains_key(&1));
        assert!(cache.piece_map.contains_key(&2));
    }

    #[test]
    fn test_reinsert_completed_piece() {
        let torrent_info = create_mock_torrent_info();
        let mut cache = PieceCache::from(torrent_info.clone());

        // Complete and remove piece 0
        let data = BytesMut::from(&[1u8; 16384][..]).freeze();
        let block = Block {
            index: 0,
            begin: 0,
            data,
        };

        let result = cache.insert_block(block);
        assert!(result.is_some());
        assert!(!cache.piece_map.contains_key(&0));

        // Try to insert another block for piece 0
        let data = BytesMut::from(&[2u8; 1024][..]).freeze();
        let block = Block {
            index: 0,
            begin: 0,
            data,
        };

        let result = cache.insert_block(block);
        assert!(result.is_none());

        // Piece should still not be in the cache
        assert!(!cache.piece_map.contains_key(&0));

        // Recreate the cache
        let new_cache = PieceCache::from(torrent_info);

        // Verify the piece exists in the new cache
        assert!(new_cache.piece_map.contains_key(&0));
    }

    #[test]
    fn insert_overlapping_blocks() {
        let torrent_info = create_mock_torrent_info();
        let mut cache = PieceCache::from(torrent_info);

        // Insert first block
        let data1 = BytesMut::from(&[1u8; 8192][..]).freeze();
        let block1 = Block {
            index: 0,
            begin: 0,
            data: data1,
        };
        cache.insert_block(block1);

        // Insert overlapping block (last 4096 bytes of first block + 4096 new bytes)
        let data2 = BytesMut::from(&[2u8; 8192][..]).freeze();
        let block2 = Block {
            index: 0,
            begin: 4096,
            data: data2,
        };
        let result = cache.insert_block(block2);

        // This should complete the piece (8192 + 8192 = 16384, which is the piece length)
        assert!(result.is_some());
        let (index, data) = result.unwrap();
        assert_eq!(index, 0);

        // First 4096 bytes should be 1
        for i in 0..4096 {
            assert_eq!(data[i], 1);
        }
        // Next 8192 bytes should be 2
        for i in 4096..12288 {
            assert_eq!(data[i], 2);
        }

        // Verify the piece has been removed from the cache
        assert!(!cache.piece_map.contains_key(&0));
    }
}
