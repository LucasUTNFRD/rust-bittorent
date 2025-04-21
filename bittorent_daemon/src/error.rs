use bittorrent_core::metainfo::TorrentError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Torrent does not exist")]
    InvalidTorrent,
    #[error("Torrent parsing error{0}")]
    TorrentParsing(TorrentError),
}
