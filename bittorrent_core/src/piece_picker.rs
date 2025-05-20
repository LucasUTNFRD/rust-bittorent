use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::{bitfield::Bitfield, metainfo::TorrentInfo};

pub struct PiecePicker {
    peer_bitifield: HashMap<SocketAddr, Bitfield>,
    total_pieces: usize,
    pieces: Vec<PieceIndex>,
}

struct PieceIndex {
    availability: usize,
    partial: bool,
    status: PieceStatus,
    // index:
}

#[derive(Eq, PartialEq)]
enum PieceStatus {
    NotRequested,
    Requested,
    Download,
}

impl From<Arc<TorrentInfo>> for PiecePicker {
    fn from(torrent: Arc<TorrentInfo>) -> Self {
        let total_pieces = torrent.get_total_pieces();
        let pieces = (0..total_pieces)
            .map(|_| PieceIndex {
                availability: 0,
                partial: false,
                status: PieceStatus::NotRequested,
            })
            .collect();
        Self {
            peer_bitifield: HashMap::new(),
            total_pieces,
            pieces,
        }
    }
}

impl PiecePicker {
    const BLOCK_SIZE: u32 = 1 << 14;

    /// register peer’s bitfield (which pieces it has)
    pub fn register_peer(&mut self, peer: SocketAddr, bitfield: Bitfield) {
        for (i, piece) in self.pieces.iter_mut().enumerate() {
            if bitfield.has_piece(i) {
                piece.availability += 1;
            }
        }
        self.peer_bitifield.insert(peer, bitfield);
    }

    pub fn update_peer(&mut self, peer: SocketAddr, piece_index: u32) {
        if let Some(bitfield) = self.peer_bitifield.get_mut(&peer) {
            //  avoid double counting
            if !bitfield.has_piece(piece_index as usize) {
                bitfield.set_piece(piece_index as usize);
                self.pieces[piece_index as usize].availability += 1;
            }
        }
    }

    pub fn unregister_peer(&mut self, peer: SocketAddr) {
        if let Some(bitfield) = self.peer_bitifield.remove(&peer) {
            for (i, piece) in self.pieces.iter_mut().enumerate() {
                // piece.availability > 0 in case of underflow
                if bitfield.has_piece(i) && piece.availability > 0 {
                    piece.availability -= 1;
                }
            }
        }
    }

    /// we are interested if peer has a piece we do not have
    pub fn check_interest(&self, peer: SocketAddr) -> bool {
        self.peer_bitifield.get(&peer).is_some_and(|peer_bf| {
            self.pieces
                .iter()
                .enumerate()
                .any(|(i, piece)| piece.status != PieceStatus::Download && peer_bf.has_piece(i))
        })
    }

    pub fn get_download_queue(&self) {
        todo!()
        // TODO:
        // we need to return a vec of block info to be sent to peer as request of pieces
        // here is were piece selection algorithm takes place
        // first four pieces are selected randomly from the pieces the peer has.
        // as soon as we receive the piece number four we switch to rarest first
        // here the priority is
        // and requested piece which was dropped by a peer and was left incompleted (Requested And Partial)
        // then a rarest piece not requested yet
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BlockInfo {
    pub index: u32,
    pub begin: u32,
    pub length: u32,
}

#[derive(Debug)]
pub struct Block {
    pub index: u32,
    pub begin: u32,
    pub block: Vec<u8>,
}
