use std::collections::BTreeMap;

use sha1::{Digest, Sha1};
use thiserror::Error;

use crate::{
    bencode::{Bencode, Encode},
    types::{InfoHash, PieceHash, PieceHashError},
};

#[derive(Debug)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
    pub info_hash: InfoHash,
}

#[derive(Debug)]
pub struct Info {
    /// size of the file in bytes, for single-file torrents
    pub length: i64,
    /// Nate to save the file / directory as
    pub name: String,
    /// number of bytes in each piece
    pub piece_length: i64,
    /// concantenated SHA-1 hashes of each piece, this will contain raw bytes
    pub pieces: Vec<PieceHash>,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum InfoError {
    #[error("Missing length field in the torrent info")]
    MissingLength,
    #[error("Missing name field in the torrent info")]
    MissingName,
    #[error("Missing piece length field in the torrent info")]
    MissingPieceLength,
    #[error("Missing pieces field in the torrent info")]
    MissingPieces,
    #[error("Piece hash error {0}")]
    PieceHash(PieceHashError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TorrentError {
    #[error("Missing announce field")]
    MissingAnnouce,
    #[error("Missing info field")]
    MissingInfo,
    #[error("Missing info: {0}")]
    MisingInfo(InfoError),
    #[error("Decoding error")]
    DecodingError,
}

const LENGTH: &[u8] = b"length";
const NAME: &[u8] = b"name";
const PIECE_LENGTH: &[u8] = b"piece length";
const PIECES: &[u8] = b"pieces";

const ANNOUNCE: &[u8] = b"announce";
const INFO: &[u8] = b"info";

impl Torrent {
    pub fn from(data: Bencode) -> Result<Torrent, TorrentError> {
        let announce_field = data.get(ANNOUNCE).ok_or(TorrentError::MissingAnnouce)?;
        let announce = match announce_field {
            Bencode::Bytes(bytes) => String::from_utf8(bytes.clone()).unwrap(),
            _ => return Err(TorrentError::MissingAnnouce),
        };

        let info_field = data.get(INFO).ok_or(TorrentError::MissingInfo)?;
        let info = match Info::from(info_field) {
            Ok(info) => info,
            Err(e) => return Err(TorrentError::MisingInfo(e)),
        };

        let info_hash = Self::calculate_info_hash(&info)?;

        Ok(Torrent {
            announce,
            info,
            info_hash,
        })
    }

    /// Calculates the InfoHash for a given Info dictionary.
    fn calculate_info_hash(info: &Info) -> Result<InfoHash, TorrentError> {
        let bencoded_info = Bencode::encode(info);

        let hash_generic_array = Sha1::digest(&bencoded_info);

        let hash_array: [u8; 20] = hash_generic_array.into();
        Ok(InfoHash::from(hash_array)) // Use the From<[u8; 20]> impl
    }

    pub fn get_announce(&self) -> &str {
        &self.announce
    }

    pub fn get_total_pieces(&self) -> u32 {
        (self.info.length as f64 / self.info.piece_length as f64).ceil() as u32
    }
}

impl Encode for Torrent {
    fn to_bencode(&self) -> Bencode {
        let mut dict = BTreeMap::new();
        dict.insert(
            ANNOUNCE.to_vec(),
            Bencode::Bytes(self.announce.as_bytes().to_vec()),
        );
        dict.insert(INFO.to_vec(), self.info.to_bencode());
        Bencode::Dict(dict)
    }
}

impl Info {
    pub fn from(info_field: &Bencode) -> Result<Info, InfoError> {
        let length_field = info_field.get(LENGTH).ok_or(InfoError::MissingLength)?;
        let length = match length_field {
            Bencode::Int(i) => *i,
            _ => return Err(InfoError::MissingLength),
        };

        let name_field = info_field.get(NAME).ok_or(InfoError::MissingName)?;
        let name = match name_field {
            Bencode::Bytes(bytes) => String::from_utf8(bytes.clone()).unwrap(),
            _ => return Err(InfoError::MissingName),
        };

        let plen_field = info_field
            .get(PIECE_LENGTH)
            .ok_or(InfoError::MissingPieceLength)?;
        let piece_length = match plen_field {
            Bencode::Int(i) => *i,
            _ => return Err(InfoError::MissingPieceLength),
        };

        let pieces_field = info_field.get(PIECES).ok_or(InfoError::MissingPieces)?;
        let pieces = match pieces_field {
            Bencode::Bytes(bytes) => {
                if bytes.len() % 20 != 0 {
                    return Err(InfoError::MissingPieces);
                }
                let hashes = bytes
                    .chunks_exact(20)
                    .map(|chunk| chunk.try_into().expect("Invalid lenght"))
                    .collect();
                hashes
            }
            _ => return Err(InfoError::MissingPieces),
        };

        Ok(Info {
            length,
            name,
            piece_length,
            pieces,
        })
    }
}

impl Encode for Info {
    fn to_bencode(&self) -> Bencode {
        let mut dict = BTreeMap::new();
        dict.insert(LENGTH.to_vec(), Bencode::Int(self.length));
        dict.insert(NAME.to_vec(), Bencode::Bytes(self.name.as_bytes().to_vec()));
        dict.insert(PIECE_LENGTH.to_vec(), Bencode::Int(self.piece_length));
        let concatendated_hashes: Vec<u8> = self
            .pieces
            .iter()
            .flat_map(|hash| hash.0.iter())
            .copied()
            .collect();
        // dbg!(&concatendated_hashes);
        dict.insert(PIECES.to_vec(), Bencode::Bytes(concatendated_hashes));
        Bencode::Dict(dict)
    }
}
