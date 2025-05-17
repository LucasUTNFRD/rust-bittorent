use std::{collections::HashSet, net::SocketAddr, time::Duration};

use futures::{SinkExt, StreamExt};
use message::{Block, Handshake, MessageDecoder};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::codec::Framed;
use tracing::info;

use crate::types::{InfoHash, PeerId};

mod message;

// ---- BITFIELD -----
struct Bitfield {
    bitfield: Vec<u8>,
    nbits: usize,
}

impl Bitfield {}

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

// handhsake -> tcpstream if ok
// new peer (tcpstream) -> peer.run.await

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum PeerConnectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("connection timed out")]
    Timeout,
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

    pub fn new(peer_addr: SocketAddr) -> Self {
        Self {
            peer_addr,
            am_interested: false,
            am_choking: true,
            remote_interested: false,
            remote_choking: false,
            outgoing_requests: HashSet::new(),
            ingoing_requests: HashSet::new(),
        }
    }

    pub async fn start(&mut self, mut stream: TcpStream) -> Result<(), PeerConnectError> {
        let decoder = MessageDecoder {};
        let mut framed_stream = Framed::new(&mut stream, decoder);

        while let Some(Ok(msg)) = framed_stream.next().await {
            info!("Peer [{}] message {:?}", self.peer_addr, msg);
        }

        todo!()
    }
}
