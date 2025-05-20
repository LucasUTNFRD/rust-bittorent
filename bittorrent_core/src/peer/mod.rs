use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use futures::{SinkExt, StreamExt};
use message::{Handshake, Message, MessageDecoder};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tokio_util::codec::Framed;
use tracing::{debug, info};

use crate::{
    bitfield::Bitfield,
    piece_picker::Block,
    torrent_session::TorrentMessage,
    types::{InfoHash, PeerId},
};

mod message;

// ---- Peer info -----
// holds information and statistics about one peer that we are connected

pub struct PeerInfo {
    // pieces: Option<Bitfield>,
    //state related
    peer_addr: SocketAddr,
    am_interested: bool,
    am_choking: bool,
    remote_interested: bool,
    remote_choking: bool,
    // request tracking
    outgoing_requests: HashSet<Block>,
    ingoing_requests: HashSet<Block>,
    //
    session_manager: mpsc::Sender<TorrentMessage>,
    our_bitfield: Arc<RwLock<Bitfield>>,
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
        our_bitfield: Arc<RwLock<Bitfield>>,
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
            our_bitfield,
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
                    info!("Peer [{}] message {:?}", self.peer_addr, msg);
                    // every we receive a message we should reset last_read_interval
                    self.handle_msg(&mut framed_stream,msg).await?;
                }
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
                // TODO:
                // Improve bitfield struct
                // try from data where
                // fails if InvalidLength or SparseBitSet
                //
                // Drop peer if:
                //     Bitfield is not sent as the first message after handshake.
                //     Length is incorrect.
                //     Spare bits are non-zero.
                // Continue session if:
                //     Bitfield is correct.
                //     Bitfield is missing (assume peer has no pieces).
                //     Bitfield under-reports possession (lazy mode).

                info!("We shuld check if we are interested");
            }
            Choke => {
                self.remote_choking = true;
                if self.am_interested {
                    self.try_request(framed_stream);
                }
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
        todo!("we should try send request for a piece to remote peer")
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
