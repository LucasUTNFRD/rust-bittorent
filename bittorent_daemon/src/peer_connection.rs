use bittorrent_core::{
    peer_protocol::{Handshake, Message, MessageDecoder},
    types::{InfoHash, PeerId},
};
use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use std::{net::SocketAddrV4, time::Duration};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{error::Elapsed, timeout},
};

pub struct PeerConnection {
    stream: TcpStream,
    peer_addr: SocketAddrV4,
    // peer_id: [u8; 20],      // The peer's peer_id
    // self_peer_id: [u8; 20], // Our peer_id

    // -- State variables --
    ///Local client is choking the remote peer
    am_choking: bool,
    /// Local client is interested in the remote peer
    am_interested: bool,
    /// Remote peer is choking the local client
    peer_choking: bool,
    /// Remote peer is interested in the local client
    peer_interested: bool,
    // -- Bitfield --
    // Stores the pieces the *remote peer* has communicated it has.
    // The size should match the total number of pieces in the torrent.
    // pub bitfield: BitField,
}

// struct PeerConnectionHandler {
// }
//

use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum PeerConnectionError {
    #[error("Connection failed {0}")]
    ConnectionFailed(std::io::Error),
    #[error("Handshake failed")]
    HandshakeFailed,
    #[error("Invalid handshake")]
    InvalidHandshake,
    #[error("Timeout connection {0}")]
    TimeoutConnection(Elapsed),
    #[error("IO error {0}")]
    Io(std::io::Error),
}

impl PeerConnection {
    pub async fn spawn(
        peer: SocketAddrV4,
        our_peer_id: PeerId,
        our_info_hash: InfoHash,
    ) -> Result<(), PeerConnectionError> {
        let mut stream = match timeout(Duration::from_secs(10), TcpStream::connect(peer)).await {
            Ok(connect_result) => match connect_result {
                Ok(s) => {
                    info!("Connected succesfully to peer:{}", peer);
                    s
                }
                Err(e) => {
                    debug!("[{}] TCP connection failed: {}", peer, e);
                    return Err(PeerConnectionError::ConnectionFailed(e));
                }
            },

            Err(e) => {
                debug!("[{}] TCP connection timed out.", e);
                return Err(PeerConnectionError::TimeoutConnection(e));
            }
        };

        // Lets perform a Peer Handhsake
        let handshake = Handshake::new(our_peer_id, our_info_hash);
        info!("Sending handshake {:?} to {}", handshake, peer);
        stream
            .write_all(&handshake.to_bytes())
            .await
            .map_err(PeerConnectionError::Io)?;

        let mut buffer = [0; 68];
        timeout(Duration::from_secs(5), stream.read_exact(&mut buffer))
            .await
            .map_err(PeerConnectionError::TimeoutConnection)?
            .map_err(PeerConnectionError::Io)?;

        if let Some(resp) = Handshake::from_bytes(&buffer) {
            if resp.info_hash != our_info_hash {
                return Err(PeerConnectionError::InvalidHandshake);
            }
            info!("Handshake successful");
        } else {
            return Err(PeerConnectionError::InvalidHandshake);
        }

        let mut peer = PeerConnection {
            stream,
            peer_addr: peer,
            am_choking: true,
            peer_choking: false,
            am_interested: false,
            peer_interested: false,
        };

        peer.run().await?;

        Ok(())
    }

    async fn run(&mut self) -> Result<(), PeerConnectionError> {
        let decoder = MessageDecoder {};
        let mut framed_stream = Framed::new(&mut self.stream, decoder);

        loop {
            tokio::select! {
                Some(msg_result) = framed_stream.next() => {
                    match msg_result{
                        Ok(msg) => {
                            match msg{
                                Message::KeepAlive => {
                                    todo!()
                                }
                                Message::Choke => {
                                    todo!()
                                }
                                Message::Unchoke => {
                                    todo!()
                                }
                                Message::Interested => {
                                    todo!()
                                }
                                Message::NotInterested => {
                                    todo!()
                                }
                                Message::Have{piece_index: _index} => {
                                    todo!()
                                }
                                Message::Bitfield(bitfield) => {
                                    todo!()
                                }
                                Message::Request(block_info) => {
                                    todo!()
                                }
                                Message::Piece(block) => {
                                    todo!()
                                }
                                Message::Cancel(block_info) => {
                                    todo!()
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Error reading message: {}", e);
                            return Err(PeerConnectionError::Io(e));
                        }
                    }

                }
            }
        }
    }
}
