use std::{collections::HashSet, net::SocketAddr, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use message::{Handshake, Message, MessageDecoder};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{broadcast, mpsc, oneshot},
};
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

use crate::{
    bitfield::{BitField, BitfieldError},
    metainfo::TorrentInfo,
    piece_picker::BlockInfo,
    torrent_session::TorrentMessage,
    types::{InfoHash, PeerId},
};

mod message;

// ---- Peer info -----
// holds information and statistics about one peer that we are connected

pub struct PeerInfo {
    //state related
    peer_addr: SocketAddr,
    am_interested: bool,
    am_choking: bool,
    remote_interested: bool,
    remote_choking: bool,
    // request tracking
    outgoing_requests: HashSet<BlockInfo>,
    ingoing_requests: HashSet<BlockInfo>,
    //
    session_manager: mpsc::Sender<TorrentMessage>,
    // TorrentInfo for valid
    torrent: Arc<TorrentInfo>,
    blocks_to_request: Option<Vec<BlockInfo>>,
    // piece_notification: broadcast::Receiver<u32>,
}

// ---- UTIL ----
pub trait ConnectTimeout {
    async fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> tokio::io::Result<TcpStream>;
}

impl ConnectTimeout for TcpStream {
    async fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> tokio::io::Result<TcpStream> {
        tokio::time::timeout(timeout, async move { TcpStream::connect(addr).await }).await?
    }
}

// ----- Connection logic -----

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum PeerConnectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    // #[error("connection timed out")]
    // Timeout,
    #[error("invalid handshake format")]
    InvalidHandshake,
    #[error("handshake fields mismatch")]
    HandshakeMismatch,
    #[error("Invalid Bitfield {0}")]
    InvalidBitfield(BitfieldError),
    #[error("Failed to send task request to session manager")]
    TaskRequestFailed,
    #[error("Session manager disconnected")]
    SessionDisconnected,
}
impl PeerInfo {
    pub async fn try_connect_to_peer(
        addr: &SocketAddr,
        peer_id: PeerId,
        info_hash: InfoHash,
    ) -> Result<TcpStream, PeerConnectError> {
        // Stablish tcp connectino to addr
        let mut stream = TcpStream::connect_timeout(addr, TIMEOUT).await?;

        //send handhsake
        let handshake = Handshake::new(peer_id, info_hash);
        stream.write_all(&handshake.to_bytes()).await?;

        // recv and validate handshake
        let mut buf = [0u8; Handshake::HANDSHAKE_LEN];
        stream.read_exact(&mut buf).await?;

        let received = Handshake::from_bytes(&buf).ok_or(PeerConnectError::InvalidHandshake)?;

        if received.info_hash != info_hash {
            return Err(PeerConnectError::HandshakeMismatch);
        }

        Ok(stream)
    }

    pub fn new(
        peer_addr: SocketAddr,
        session_manager: mpsc::Sender<TorrentMessage>,
        torrent: Arc<TorrentInfo>,
        // piece_update: broadcast::Receiver<u32>, // if we receive some broadcast notify peer a piece we have
    ) -> Self {
        Self {
            peer_addr,
            am_interested: false,
            am_choking: true,
            remote_interested: false,
            remote_choking: false,
            outgoing_requests: HashSet::new(),
            ingoing_requests: HashSet::new(),
            session_manager,
            torrent,
            blocks_to_request: None,
        }
    }

    pub async fn start(
        &mut self,
        mut stream: TcpStream,
        our_bitield: Option<BitField>,
    ) -> Result<(), PeerConnectError> {
        let decoder = MessageDecoder {};
        let mut framed_stream = Framed::new(&mut stream, decoder);

        if let Some(bitfield) = our_bitield {
            info!("Sending our bitfield to peer [{}]", self.peer_addr);
            framed_stream
                .send(Message::Bitfield(bitfield.as_bytes()))
                .await
                .map_err(PeerConnectError::Io)?;
        }

        loop {
            tokio::select! {
                Some(msg) = framed_stream.next() =>{
                    let msg = msg?;
                    // every we receive a message we should reset last_read_interval
                    self.handle_msg(&mut framed_stream, msg).await?;
                }
                // cmd = cmd_tx.recv() =>{
                    // match cmd{
                    //  Task
                    //
                    // }

                // }
            }
        }
    }

    async fn handle_msg(
        &mut self,
        framed_stream: &mut Framed<&mut TcpStream, MessageDecoder>,
        msg: Message,
    ) -> Result<(), PeerConnectError> {
        use Message::*;
        match msg {
            KeepAlive => {
                debug!("[{}] sent Keep Alive", self.peer_addr);
            }
            Bitfield(data) => {
                debug!("[{}] sent bitfield", self.peer_addr);
                let num_pieces = self.torrent.get_total_pieces();
                match BitField::try_from(data, num_pieces) {
                    Ok(bitfield) => {
                        let (ask_interest, resp) = oneshot::channel();
                        let _ = self
                            .session_manager
                            .send(TorrentMessage::PeerBitfield {
                                peer_addr: self.peer_addr,
                                peer_bf: bitfield,
                                interest: ask_interest,
                            })
                            .await;
                        if let Ok(am_interested) = resp.await {
                            if am_interested {
                                debug!("We are interested to {}", self.peer_addr);
                                let _ = framed_stream.send(Message::Interested).await;
                                self.am_interested = true;
                                self.try_request(framed_stream).await?;
                            } /* intially we are not interested*/
                        }
                    }
                    Err(e) => return Err(PeerConnectError::InvalidBitfield(e)),
                };
            }
            Choke => {
                debug!("[{}] choked us", self.peer_addr);
                self.remote_choking = true;
                // This blocks are dropped
                // TODO: We want block grained piece selection
                // we should track this
                // self.outgoing_requests.clear();
            }
            Unchoke => {
                self.remote_choking = false;
                debug!("Peer {} is willing to send data ", self.peer_addr);
                self.try_request(framed_stream).await?;
            }
            Interested => {
                self.remote_interested = true;
                todo!("We did not send our bitfield")
            }
            NotInterested => {
                self.remote_interested = false;
                todo!("We did not send our bitfield")
            }
            Have { piece_index } => {
                // increment piece avalability

                if self.am_interested {
                    self.session_manager
                        .send(TorrentMessage::PeerHave(self.peer_addr, piece_index, None))
                        .await
                        .map_err(|_| PeerConnectError::SessionDisconnected)?;
                } else {
                    debug!("Peer now has:{} re-check interest", piece_index);
                    let (ask_interest, resp) = oneshot::channel();
                    self.session_manager
                        .send(TorrentMessage::PeerHave(
                            self.peer_addr,
                            piece_index,
                            Some(ask_interest),
                        ))
                        .await
                        .map_err(|_| PeerConnectError::SessionDisconnected)?;
                    if let Ok(am_interested) = resp.await {
                        if am_interested {
                            debug!("We are interested to {}", self.peer_addr);
                            let _ = framed_stream.send(Message::Interested).await;
                            self.am_interested = true;
                            self.try_request(framed_stream).await?;
                        } else {
                            debug!("We are not  interested to {}", self.peer_addr);
                        }
                    }
                }

                // if we are not interested re-check interest
            }
            Request(_block_info) => {
                // we manage at most 5 request x peer
                // if can send  then
                // let block = self.read_block(block_info);
                // framed_stream.send(Piece(block)).await?;
                // else
                //
            }
            Piece(block) => {
                let info = BlockInfo {
                    index: block.index,
                    begin: block.begin,
                    length: block.data.len() as u32,
                };

                if !self.outgoing_requests.contains(&info) {
                    warn!("Received an unrequested piece");
                    return Ok(());
                }
                self.outgoing_requests.remove(&info);
                self.session_manager
                    .send(TorrentMessage::Piece(self.peer_addr, block))
                    .await
                    .map_err(|_| PeerConnectError::SessionDisconnected)?;
                self.try_request(framed_stream).await?;
            }
            Cancel(_block_info) => {
                // if that block was previously requested by peer unmark it from peer_requests
                // abort reading?
            }
        }

        Ok(())
    }

    const THRESHOLD: usize = 5;
    async fn try_request(
        &mut self,
        framed_stream: &mut Framed<&mut TcpStream, MessageDecoder>,
    ) -> Result<(), PeerConnectError> {
        if !self.remote_choking && self.am_interested {
            if self.outgoing_requests.len() >= Self::THRESHOLD {
                return Ok(());
            }
            debug!("Trying to request data");
            if self.blocks_to_request.is_none() {
                self.get_tasks().await?;
            }
            if let Some(blocks_to_request) = self.blocks_to_request.as_mut() {
                while let Some(block_info) = blocks_to_request.pop() {
                    debug!("Requesting {:?} to Peer [{}]", block_info, self.peer_addr);
                    framed_stream
                        .send(Message::Request(block_info))
                        .await
                        .map_err(PeerConnectError::Io)?;
                    self.outgoing_requests.insert(block_info);
                    if self.outgoing_requests.len() >= Self::THRESHOLD {
                        debug!("Pipeline filled for peer {}", self.peer_addr);
                        break;
                    }
                }
            } else {
                debug!("Nothing to do now");
            }
        }
        Ok(())
    }

    async fn get_tasks(&mut self) -> Result<(), PeerConnectError> {
        let (task_tx, task_rx) = oneshot::channel();

        self.session_manager
            .send(TorrentMessage::GetTask(self.peer_addr, task_tx))
            .await
            .map_err(|_| PeerConnectError::SessionDisconnected)?;

        self.blocks_to_request = task_rx
            .await
            .map_err(|_| PeerConnectError::TaskRequestFailed)?;
        debug!(
            "Rec response from torrent manager {:?}",
            self.blocks_to_request
        );
        Ok(())
    }
}
