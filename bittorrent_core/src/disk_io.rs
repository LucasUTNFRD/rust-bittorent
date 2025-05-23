use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::Arc,
};

use bytes::Bytes;
use tokio::{sync::mpsc, task::JoinError};
use tracing::info;

use crate::types::InfoHash;

pub enum IOMessage {
    ReadBlock, // info_hash, file_offset, how many bytes read
    WriteBlock {
        info_hash: InfoHash,
        offset: u64,
        data: Bytes,
    },
    RegisterTorrent(InfoHash, File),
}

pub struct DiskHandle {
    sender: mpsc::Sender<IOMessage>,
}

struct DiskActor {
    receiver: mpsc::Receiver<IOMessage>,
    torrents: HashMap<InfoHash, Arc<File>>,
}

#[derive(Debug, thiserror::Error)]
pub enum DiskIOError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Torrent not registered: {0}")]
    TorrentNotRegistered(InfoHash),
    #[error("Channel disconnected")]
    ChannelDisconnected,
    #[error("Join error{0}")]
    JoinError(JoinError),
}

impl DiskActor {
    pub fn new(receiver: mpsc::Receiver<IOMessage>) -> Self {
        Self {
            receiver,
            torrents: HashMap::new(),
        }
    }

    pub async fn run(mut actor: DiskActor) {
        while let Some(event) = actor.receiver.recv().await {
            match event {
                IOMessage::ReadBlock => actor.read_piece(),
                IOMessage::WriteBlock {
                    info_hash,
                    offset,
                    data,
                } => {
                    actor.write_piece(info_hash, offset, data).unwrap();
                }
                IOMessage::RegisterTorrent(info_hash, path) => {
                    actor.torrents.insert(info_hash, Arc::new(path));
                }
            }
        }
    }

    fn write_piece(
        &self,
        info_hash: InfoHash,
        offset: u64,
        data: Bytes,
    ) -> Result<(), DiskIOError> {
        let file = self
            .torrents
            .get(&info_hash)
            .ok_or(DiskIOError::TorrentNotRegistered(info_hash))?
            .clone();

        tokio::task::spawn_blocking(move || {
            let mut file = file.as_ref();
            file.seek(SeekFrom::Start(offset)).unwrap();
            file.write_all(&data).unwrap();
        });

        Ok(())
    }

    fn read_piece(&self) {
        let mut output_file = File::open("some_file").unwrap();
        let start_idx = 0;
        let length = 0;
        tokio::task::spawn_blocking(move || {
            output_file
                .seek(std::io::SeekFrom::Start(start_idx))
                .unwrap();
            let mut buffer = vec![0; length as usize];
            output_file.read_exact(&mut buffer).unwrap();
        });
    }
}

impl DiskHandle {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(10000);
        let actor = DiskActor::new(receiver);
        info!("Starting Disk Actor");
        tokio::task::spawn(DiskActor::run(actor));

        Self { sender }
    }

    pub async fn send(&self, message: IOMessage) {
        let _ = self.sender.send(message).await;
    }

    pub async fn register_torrent(&self, info_hash: InfoHash, filename: String, file_size: u64) {
        let download_dir = get_download_dir().expect("FAILED TO GET DOWNLOAD DIR");
        tokio::fs::create_dir_all(&download_dir)
            .await
            .expect("Failed to create dir");

        let full_path = download_dir.join(filename);

        // Create and pre-allocate the file in a blocking task
        let file = tokio::task::spawn_blocking(move || {
            let file = std::fs::File::create(&full_path).expect("Failed to create file");
            file.set_len(file_size).expect("Failed to set file length");
            file // Return the raw File
        })
        .await
        .expect("Failed to spawn blocking task");

        self.send(IOMessage::RegisterTorrent(info_hash, file)).await;
    }
}

fn get_download_dir() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        let mut path = PathBuf::from(home);
        path.push("Downloads/Torrents");
        Some(path)
    } else {
        None
    }
}
