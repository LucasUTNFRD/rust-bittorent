use std::{collections::HashSet, net::SocketAddr, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use message::{Handshake, Message, MessageDecoder};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, oneshot},
};
use tokio_util::codec::Framed;
use tracing::{debug, info};

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
        }
    }

    pub async fn start(&mut self, mut stream: TcpStream) -> Result<(), PeerConnectError> {
        let decoder = MessageDecoder {};
        let mut framed_stream = Framed::new(&mut stream, decoder);

        {
            // The bitfield message may only be sent immediately after the handshaking sequence is completed, and before any other messages are sent.
            // It is optional, and need not be sent if a client has no pieces.
            // let bitfield = self.our_bitfield.read().unwrap().clone();

            info!("Sending our bitfield to peer [{}]", self.peer_addr);
            // framed_stream.send(Message::Bitfield(bitfield)).await?;
        };

        loop {
            tokio::select! {
                Some(msg) = framed_stream.next() => {
                    let msg = msg?;
                    // every we receive a message we should reset last_read_interval
                    self.handle_msg(&mut framed_stream,msg).await?;
                }
                // recv a task that we requested
                // broadcast of piece obtained
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
                // Drop peer if:
                //     Bitfield is not sent as the first message after handshake.
                //     Length is incorrect.
                //     Spare bits are non-zero.
                // Continue session if:
                //     Bitfield is correct.
                //     Bitfield is missing (assume peer has no pieces).
                //     Bitfield under-reports possession (lazy mode).
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
                                let _ = framed_stream.send(Message::Interested).await;
                                self.am_interested = true;
                                self.try_request(framed_stream);
                            } /* intially we are not interested*/
                        }
                    }
                    Err(e) => return Err(PeerConnectError::InvalidBitfield(e)),
                };
            }
            Choke => {
                self.remote_choking = true;
            }
            Unchoke => {
                self.remote_choking = false;
                self.cancel_pending_requests(framed_stream);
            }
            Interested => {
                self.remote_interested = true;
                //this info is useful for choking logic
                todo!()
            }
            NotInterested => {
                self.remote_interested = false;
                //this info is useful for choking logic
                todo!()
            }
            Have { piece_index } => {
                // increment piece avalability
                todo!()
            }
            Request(block_info) => {
                // we manage at most 5 request x peer
                // if can send  then
                // let block = self.read_block(block_info);
                // framed_stream.send(Piece(block)).await?;
                // else
                //
            }
            Piece(block) => {
                // check if we requested this block
                // if yes then
                // unmark as a requested
                // and sha1 validation
                // and write piece
                // else ignore or track this peer is sending wrong pieces
            }
            Cancel(block_info) => {
                // if that block was previously requested by peer unmark it from peer_requests
                // abort reading?
            }
        }

        Ok(())
    }

    fn try_request(&mut self, framed_stream: &mut Framed<&mut TcpStream, MessageDecoder>) {
        if !self.remote_choking && self.am_interested {
            // let tasks: Vec<Option<BlockInfo>>;
            // while let Some(block_to_request) = tasks.pop() {
            //     // mando
            //     framed_stream.send(Message::Request(block_to_request));
            //     // marco
            //     self.outgoing_requests.insert(block_to_request);
            //     // me fijo si quedan tareas
            //     if !remainding_tasks() {
            //         get_more_tasks(self.peer_addr);
            //     }
            //     // me fijo si nos da el threshold para mandar pipeline de requesdt
            //     if self.outgoing_requests.len() >= THRESHOLD {
            //         break;
            //     }
            // }
        }
    }

    fn cancel_pending_requests(
        &mut self,
        framed_stream: &mut Framed<&mut TcpStream, MessageDecoder>,
    ) {
        todo!("we should send cancel request for pending request we did to remote peer")
    }
}

// question to resolve
// how does PeerInfo become aware of a piece acquistion?
// what we do with bitfield?
// how we select a piece to request?
