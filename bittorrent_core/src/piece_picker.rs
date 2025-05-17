use std::sync::Arc;

use crate::metainfo::TorrentInfo;

struct PiecePicker {
    torrent: Arc<TorrentInfo>,
}
