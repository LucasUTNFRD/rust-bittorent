use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use tokio::sync::mpsc;
use tracing::info;

use crate::types::InfoHash;

pub enum IOMessage {
    // NewTorrent(InfoHash),
    ReadBlock,  // info_hash, file_offset, how many bytes read
    WriteBlock, // info_hash, file_offset , byte to write
    RegisterTorrent(InfoHash, PathBuf),
}

pub struct DiskHandle {
    sender: mpsc::Sender<IOMessage>,
}

struct DiskActor {
    receiver: mpsc::Receiver<IOMessage>,
    torrents: HashMap<InfoHash, PathBuf>,
}

const DEFAULT_FOLDER: &str = "$HOME/Downloads/Torrent/";

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
                IOMessage::WriteBlock => actor.write_piece(),
                IOMessage::RegisterTorrent(info_hash, path) => {
                    actor.torrents.insert(info_hash, path);
                }
            }
        }
    }

    // file system manipulation fn
    fn write_piece(&self) {
        let mut output_file = File::open("some_file").unwrap();
        let start_idx = 0;
        let piece_data = [0, 0, 0];
        tokio::task::spawn_blocking(move || {
            // TODO: Remove unwraps
            output_file.seek(SeekFrom::Start(start_idx)).unwrap();
            output_file.write_all(&piece_data).unwrap();
        });
    }

    fn read_piece(&self) {
        let mut output_file = File::open("some_file").unwrap();
        let start_idx = 0;
        let length = 0;
        tokio::task::spawn_blocking(move || {
            // TODO: Remove unwraps
            output_file
                .seek(std::io::SeekFrom::Start(start_idx))
                .unwrap();
            let mut buffer = vec![0; length as usize];
            output_file.read_exact(&mut buffer).unwrap();
            // TODO:
            // send read buffer to torrent handler
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

    pub async fn register_torrent(&self, info_hash: InfoHash, name: String) {
        let path = PathBuf::from(DEFAULT_FOLDER).join(name);
        tokio::fs::create_dir_all(&path)
            .await
            .expect("Failed to create dir");
        self.send(IOMessage::RegisterTorrent(info_hash, path)).await;
    }
}
