use std::net::SocketAddrV4;

use tokio::net::TcpStream;

pub struct PeerConnection {
    stream: TcpStream,
    peer_addr: SocketAddrV4,
    // peer_id: [u8; 20],      // The peer's peer_id
    self_peer_id: [u8; 20], // Our peer_id

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

struct PeerConnectionHandler {}
