use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use bytes::Bytes;
use rand::{rng, seq::IteratorRandom};
use tracing::{debug, info, warn};

use crate::{bitfield::BitField, metainfo::TorrentInfo};

/// This data structure has view of what peer has
pub struct PiecePicker {
    peer_bitifield: HashMap<SocketAddr, BitField>,
    total_pieces: usize,
    pieces: Vec<PieceIndex>,
    pick_strategy: Strategy,
}

#[derive(Debug)]
struct PieceIndex {
    availability: usize,
    partial: bool,
    status: PieceStatus,
    size: u32,
    // index: usize
    // blocks: Vec<BlockInfo>
}

#[derive(Debug, Eq, PartialEq)]
pub enum PieceStatus {
    NotRequested,
    Requested,
    Download,
}

#[derive(Eq, PartialEq)]
enum Strategy {
    RandomFirst,
    RarestFirst,
}

impl From<Arc<TorrentInfo>> for PiecePicker {
    fn from(torrent: Arc<TorrentInfo>) -> Self {
        let total_pieces = torrent.get_total_pieces();
        let pieces = (0..total_pieces)
            .map(|piece_idx| PieceIndex {
                availability: 0,
                partial: false,
                status: PieceStatus::NotRequested,
                size: torrent.get_piece_len(piece_idx),
            })
            .collect();
        Self {
            peer_bitifield: HashMap::new(),
            total_pieces,
            pieces,
            pick_strategy: Strategy::RandomFirst,
        }
    }
}

impl PiecePicker {
    const BLOCK_SIZE: u32 = 1 << 14;

    /// register peer’s bitfield (which pieces it has)
    pub fn register_peer(&mut self, peer: SocketAddr, bitfield: BitField) {
        for (i, piece) in self.pieces.iter_mut().enumerate() {
            if bitfield.has_piece(i) {
                piece.availability += 1;
            }
        }
        self.peer_bitifield.insert(peer, bitfield);
    }

    pub fn update_peer(&mut self, peer: &SocketAddr, piece_index: u32) {
        if let Some(bitfield) = self.peer_bitifield.get_mut(peer) {
            //  avoid double counting
            if !bitfield.has_piece(piece_index as usize) {
                bitfield.set_piece(piece_index as usize);
                self.pieces[piece_index as usize].availability += 1;
            }
        } else {
            warn!(
                "Received a have message and didnt have the peeer bf, we should create an empty bf all zeroed"
            );
        }
    }

    // pub fn mark_piece(&mut self, )

    pub fn unregister_peer(&mut self, peer: &SocketAddr) {
        if let Some(bitfield) = self.peer_bitifield.remove(peer) {
            for (i, piece) in self.pieces.iter_mut().enumerate() {
                // piece.availability > 0 in case of underflow
                if bitfield.has_piece(i) && piece.availability > 0 {
                    piece.availability -= 1;
                }
            }
        }
    }

    fn mark_piece(&mut self, piece_idx: usize, new_state: PieceStatus) {
        if let Some(piece) = self.pieces.get_mut(piece_idx) {
            debug!("piece {} {:?} -> {:?}", piece_idx, piece.status, new_state);
            piece.status = new_state;
        }
    }

    pub fn mark_piece_downloaded(&mut self, piece_idx: usize) {
        self.mark_piece(piece_idx, PieceStatus::Download);
        if self.pick_strategy != Strategy::RarestFirst
            && self
                .pieces
                .iter()
                .filter(|&p| p.status == PieceStatus::Download)
                .count()
                == 4
        {
            info!("Rarest First strategy");
            self.pick_strategy = Strategy::RarestFirst
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

    pub fn all_pieces_downloaded(&self) -> bool {
        self.pieces
            .iter()
            .all(|p| p.status == PieceStatus::Download)
    }

    fn get_blocks(&self, piece_idx: u32) -> Vec<BlockInfo> {
        let piece_size = self.pieces[piece_idx as usize].size;
        (0..piece_size)
            .step_by(Self::BLOCK_SIZE as usize)
            .map(|offset| BlockInfo {
                index: piece_idx,
                begin: offset,
                length: Self::BLOCK_SIZE.min(piece_size - offset),
            })
            .collect()
    }

    pub fn pick_piece(&mut self, peer: &SocketAddr) -> Option<Vec<BlockInfo>> {
        let peer_bitfield = self.peer_bitifield.get(peer)?;
        let candidate_pieces: Vec<usize> = self
            .pieces
            .iter()
            .enumerate()
            .filter(|&(i, p)| p.status == PieceStatus::NotRequested && peer_bitfield.has_piece(i)) // piece not request and peer has
            .map(|(i, _)| i)
            .collect();
        debug!("Candidate pieces {:?}", candidate_pieces);
        let piece_index = match self.pick_strategy {
            Strategy::RandomFirst => candidate_pieces.iter().choose(&mut rng())?,
            Strategy::RarestFirst => candidate_pieces
                .iter()
                .min_by_key(|&i| self.pieces[*i].availability)?,
        };

        // NotRequested -> Requested
        self.mark_piece(*piece_index, PieceStatus::Requested);

        // Some(self.pieces[*piece_index].blocks)
        Some(self.get_blocks(*piece_index as u32))
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct BlockInfo {
    pub index: u32,
    pub begin: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Block {
    pub index: u32,
    pub begin: u32,
    pub data: Bytes,
}
