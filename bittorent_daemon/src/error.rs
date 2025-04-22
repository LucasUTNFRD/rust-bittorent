use std::io::Error;

use bincode::error::DecodeError;
use bittorrent_core::{metainfo::TorrentError, torrent_parser::ParseError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Torrent does not exist")]
    InvalidTorrent,
    #[error("Torrent parsing error{0}")]
    TorrentParsing(ParseError),
    #[error("IO related err:{0}")]
    IO(Error),
    #[error("Decode related err:{0}")]
    Decode(DecodeError),
}
