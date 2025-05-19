use std::{
    io::{Error, Read},
    path::Path,
};

use thiserror::Error;

use crate::{
    bencode::{Bencode, BencodeError},
    metainfo::{TorrentError, TorrentInfo},
};

pub struct TorrentParser;

#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("IO error: {0}")]
    IOError(#[from] Error),
    #[error("Bencode error: {0}")]
    BencodeError(#[from] BencodeError),
    #[error("TorrentInfo error: {0}")]
    TorrentError(#[from] TorrentError),
}

impl TorrentParser {
    pub fn parse(path: &Path) -> Result<TorrentInfo, ParseError> {
        let data = match TorrentParser::read_file(path) {
            Ok(data) => data,
            Err(e) => return Err(ParseError::IOError(e)),
        };

        let bencoded_data = match Bencode::decode(&data) {
            Ok(data) => data,
            Err(e) => return Err(ParseError::BencodeError(e)),
        };

        let torrent = match TorrentInfo::try_from(bencoded_data) {
            Ok(torrent) => torrent,
            Err(e) => return Err(ParseError::TorrentError(e)),
        };

        Ok(torrent)
    }

    fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = Vec::new();

        reader.read_to_end(&mut buffer)?;

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use super::*;

    #[test]
    fn parse_sample_torrent() {
        let manifest_dir =
            env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR env var not set during test");
        let mut filepath = PathBuf::from(manifest_dir);

        assert!(
            filepath.pop(),
            "Failed to navigate to workspace root from manifest dir"
        );

        filepath.push("sample_torrents");
        filepath.push("sample.torrent");

        println!("Attempting to load test file: {}", filepath.display());
        assert!(
            filepath.exists(),
            "Test torrent file not found at: {}",
            filepath.display()
        );

        let torrent = TorrentParser::parse(&filepath).expect("Failed to parse sample.torrent file");
        let expected_tracker_url = "http://bittorrent-test-tracker.codecrafters.io/announce";
        let length = 92063;
        let expected_info_hash = "d69f91e6b2ae4c542468d1073a71d4ea13879a7f";
        assert_eq!(torrent.announce, expected_tracker_url.to_string());
        assert_eq!(torrent.info_hash.to_hex(), expected_info_hash);
        assert_eq!(torrent.info.length, length);
    }
}
