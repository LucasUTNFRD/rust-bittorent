use tokio::sync::mpsc;
use tracing::info;

use crate::types::InfoHash;

pub enum IOMessage {
    // NewTorrent(InfoHash),
    ReadBlock,  // info_hash, file_offset, how many bytes read
    WriteBlock, // info_hash, file_offset , byte to write
    Validate,
}

pub struct DiskHandle {
    sender: mpsc::Sender<IOMessage>,
}

struct DiskActor {
    receiver: mpsc::Receiver<IOMessage>,
    // torrent: HashMap<InfoHash,Path>
}

const DEFAULT_FOLDER: &str = "$HOME/Downloads/Torrent/";

impl DiskActor {
    pub fn new(receiver: mpsc::Receiver<IOMessage>) -> Self {
        Self { receiver }
    }

    pub fn run(mut actor: DiskActor) {
        while let Some(event) = actor.receiver.blocking_recv() {
            match event {
                IOMessage::ReadBlock => todo!(),
                IOMessage::WriteBlock => todo!(),
                IOMessage::Validate => todo!(),
            }
        }
    }
}

impl DiskHandle {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(10000);
        let actor = DiskActor::new(receiver);
        info!("Starting Disk Actor");
        tokio::task::spawn_blocking(|| DiskActor::run(actor));

        Self { sender }
    }

    pub async fn send(&self, message: IOMessage) {
        let _ = self.sender.send(message).await;
    }
}

// TODO: Test this actor
