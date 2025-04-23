use bytes::{BufMut, BytesMut};

use crate::types::{InfoHash, PeerId};

pub enum Message {
    Handshake {
        peer_id: PeerId,
        info_hash: InfoHash,
    },
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have {
        piece_index: u32,
    },
    Bitfield(Bitfield),
    Request(BlockInfo),
    Piece(Block),
    Cancel(BlockInfo),
    Port {
        listen_port: u16,
    },
}

pub struct BlockInfo {
    index: u32,
    begin: u32,
    length: u32,
}

pub struct Block {
    index: u32,
    begin: u32,
    block: Vec<u8>,
}

#[derive(Debug, Clone)]
struct Bitfield {
    bitvec: Vec<u8>,
}

const PSTRLEN: u8 = 19;
const PSTR: &[u8; 19] = b"BitTorrent protocol";

impl Message {
    pub fn to_bytes(&self, buf: &mut BytesMut) {
        match self {
            Message::Handshake { peer_id, info_hash } => {
                // <pstrlen><pstr><reserved><info_hash><peer_id>
                buf.put_u8(PSTRLEN);
                buf.put_slice(PSTR);
                // put 8 reserved bytes, use all zeroes
                let reserved = [0u8; 8];
                buf.put_slice(&reserved);
                buf.put_slice(&info_hash.as_bytes());
                buf.put_slice(&peer_id.0);
            }
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
            Message::Have { piece_index } => {
                todo!()
            }
            Message::Bitfield(bitfield) => {
                todo!()
            }
            Message::Request(block_info) => {
                todo!()
            }
            Message::Piece(received_block) => {
                todo!()
            }
            Message::Cancel(block_info) => {
                todo!()
            }
            Message::Port { listen_port } => {
                todo!()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn handshake_serialization() {
        let peer_id = PeerId(*b"-TR2920-abcdefghijkl");
        let info_hash = InfoHash([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        ]);
        let handshake = Message::Handshake {
            peer_id: peer_id,
            info_hash: info_hash,
        };
        let mut buf = BytesMut::with_capacity(68); // Handshake is exactly 68 bytes
        handshake.to_bytes(&mut buf);

        // Verify the structure of the handshake:
        // 1. First byte is pstrlen (19)
        assert_eq!(buf[0], PSTRLEN);

        // 2. Next 19 bytes should be the protocol string
        assert_eq!(&buf[1..20], PSTR);

        // 3. Next 8 bytes are reserved (all zeros)
        for i in 20..28 {
            assert_eq!(buf[i], 0);
        }

        // 4. Next 20 bytes should be the info_hash
        assert_eq!(&buf[28..48], &info_hash.0);

        // 5. Final 20 bytes should be the peer_id
        assert_eq!(&buf[48..68], &peer_id.0);

        // The total buffer should be exactly 68 bytes
        assert_eq!(buf.len(), 68);
    }
}
