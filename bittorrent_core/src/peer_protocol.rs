use bytes::{Buf, BufMut, BytesMut};

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
    Bitfield(Vec<u8>),
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

const PSTRLEN: u8 = 19;
const PSTR: &[u8; 19] = b"BitTorrent protocol";
const HANDHSAKE_LEN: usize = 68;

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
            //  <length prefix><message ID><payload>
            Message::KeepAlive => {
                buf.put_u32(0);
            }
            Message::Choke => {
                buf.put_u32(1); // LEN:[0,0,0,1]
                buf.put_u8(0); // ID: 0
            }
            Message::Unchoke => {
                buf.put_u32(1); // LEN:[0,0,0,1]
                buf.put_u8(1); // ID: 1
            }
            Message::Interested => {
                buf.put_u32(1); // LEN:[0,0,0,1]
                buf.put_u8(2); // ID: 2
            }
            Message::NotInterested => {
                buf.put_u32(1); // LEN:[0,0,0,1]
                buf.put_u8(3); // ID: 3
            }
            Message::Have { piece_index } => {
                // have: <len=0005><id=4><piece index>
                buf.put_u32(5); // LEN:[0,0,0,5]
                buf.put_u8(4); // ID: 4
                buf.put_u32(*piece_index);
            }
            Message::Bitfield(bitfield) => {
                // bitfield: <len=0001+X><id=5><bitfield>
                let bitfield_len = bitfield.len();
                buf.put_u32(1 + bitfield_len as u32); // LEN:[0,0,0,1+X]
                buf.put_u8(5); // ID: 5
                buf.put_slice(bitfield);
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

    pub fn from_bytes(src: &mut BytesMut) -> Option<Self> {
        if src.len() == HANDHSAKE_LEN {
            let ptrlen = src.get_u8();
            assert_eq!(PSTRLEN, ptrlen);

            let mut protocol_str = [0u8; 19];
            src.copy_to_slice(&mut protocol_str);
            assert_eq!(protocol_str, *PSTR);
            let _reserved = src.copy_to_bytes(8);

            let mut info_hash = [0u8; 20];
            src.copy_to_slice(&mut info_hash);
            let info_hash = InfoHash(info_hash);
            let mut peer_id_bytes = [0u8; 20];
            src.copy_to_slice(&mut peer_id_bytes);
            let peer_id = PeerId(peer_id_bytes);

            return Some(Message::Handshake { peer_id, info_hash });
        }

        unimplemented!()
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
        let handshake = Message::Handshake { peer_id, info_hash };
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

    #[test]
    fn keep_alive_serialization() {
        let keep_alive = Message::KeepAlive;
        let mut buf = BytesMut::with_capacity(4); // KeepAlive is exactly 4 bytes
        keep_alive.to_bytes(&mut buf);

        // Verify the structure of the keep_alive:
        // 1. First 4 bytes should be the length prefix (0)
        assert_eq!(buf[0..4], [0, 0, 0, 0]);

        // The total buffer should be exactly 4 bytes
        assert_eq!(buf.len(), 4);
    }

    #[test]
    fn choke_serialization() {
        let choke = Message::Choke;
        let mut buf = BytesMut::with_capacity(5); // Choke is exactly 5 bytes
        choke.to_bytes(&mut buf);

        // Verify the structure of the choke:
        // 1. First 4 bytes should be the length prefix (1)
        assert_eq!(buf[0..4], [0, 0, 0, 1]);

        // 2. Next byte should be the ID (0)
        assert_eq!(buf[4], 0);

        // The total buffer should be exactly 5 bytes
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn unchoke_serialization() {
        let unchoke = Message::Unchoke;
        let mut buf = BytesMut::with_capacity(5); // Unchoke is exactly 5 bytes
        unchoke.to_bytes(&mut buf);

        // Verify the structure of the unchoke:
        // 1. First 4 bytes should be the length prefix (1)
        assert_eq!(buf[0..4], [0, 0, 0, 1]);

        // 2. Next byte should be the ID (1)
        assert_eq!(buf[4], 1);

        // The total buffer should be exactly 5 bytes
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn interested_serialization() {
        let interested = Message::Interested;
        let mut buf = BytesMut::with_capacity(5); // Interested is exactly 5 bytes
        interested.to_bytes(&mut buf);

        // Verify the structure of the interested:
        // 1. First 4 bytes should be the length prefix (1)
        assert_eq!(buf[0..4], [0, 0, 0, 1]);

        // 2. Next byte should be the ID (2)
        assert_eq!(buf[4], 2);

        // The total buffer should be exactly 5 bytes
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn not_interested_serialization() {
        let not_interested = Message::NotInterested;
        let mut buf = BytesMut::with_capacity(5); // NotInterested is exactly 5 bytes
        not_interested.to_bytes(&mut buf);

        // Verify the structure of the not_interested:
        // 1. First 4 bytes should be the length prefix (1)
        assert_eq!(buf[0..4], [0, 0, 0, 1]);

        // 2. Next byte should be the ID (3)
        assert_eq!(buf[4], 3);

        // The total buffer should be exactly 5 bytes
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn bitfield_serialization() {
        let bitfield = vec![0, 1, 2, 3];
        let message = Message::Bitfield(bitfield);
        let mut buf = BytesMut::with_capacity(9); // Bitfield is exactly 9 bytes
        message.to_bytes(&mut buf);

        // Verify the structure of the bitfield:
        // 1. First 4 bytes should be the length prefix (5)
        assert_eq!(buf[0..4], [0, 0, 0, 5]);

        // 2. Next byte should be the ID (5)
        assert_eq!(buf[4], 5);

        // 3. Next 4 bytes should be the bitfield data (0x01020304)
        assert_eq!(buf[5..9], [0, 1, 2, 3]);

        // The total buffer should be exactly 9 bytes
        assert_eq!(buf.len(), 9);
    }

    #[test]
    fn test_handshake_deserialize() {
        // Create a valid handshake message in bytes
        let peer_id = PeerId(*b"-TR2920-abcdefghijkl");
        let info_hash = InfoHash([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        ]);

        // Create a buffer with the handshake message
        let mut buf = BytesMut::with_capacity(68);
        buf.put_u8(PSTRLEN); // pstrlen
        buf.put_slice(PSTR); // protocol string
        buf.put_bytes(0, 8); // reserved bytes
        buf.put_slice(&info_hash.0); // info_hash
        buf.put_slice(&peer_id.0); // peer_id

        // Decode the handshake
        let message = Message::from_bytes(&mut buf);

        // Check if the message was decoded correctly
        assert!(message.is_some());
        if let Some(Message::Handshake {
            peer_id: decoded_peer_id,
            info_hash: decoded_info_hash,
        }) = message
        {
            assert_eq!(decoded_peer_id.0, peer_id.0);
            assert_eq!(decoded_info_hash.0, info_hash.0);
        } else {
            panic!("Decoded message is not a handshake!");
        }

        // The buffer should be empty after decoding
        assert_eq!(buf.len(), 0);
    }
}
