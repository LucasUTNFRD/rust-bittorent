use bytes::Bytes;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitField {
    total_pieces: usize,
    inner: Vec<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BitfieldError {
    #[error("Invalid Length expected{expected_len}, got {actual_len}")]
    InvalidLength {
        expected_len: usize,
        actual_len: usize,
    },
    #[error("Non zero spare bits")]
    NonZeroSpareBits,
}

impl BitField {
    pub fn new(total_pieces: usize) -> Self {
        let num_bytes = (total_pieces + 7) / 8;
        Self {
            total_pieces,
            inner: vec![0u8; num_bytes],
        }
    }

    pub fn try_from(bytes: Bytes, num_pieces: usize) -> Result<Self, BitfieldError> {
        let expected_bytes = (num_pieces + 7) / 8;

        if bytes.len() < expected_bytes {
            return Err(BitfieldError::InvalidLength {
                expected_len: expected_bytes,
                actual_len: bytes.len(),
            });
        }

        // Check spare bits in the last byte
        let last_byte_bits = num_pieces % 8;
        if last_byte_bits != 0 {
            // If num_pieces is not a multiple of 8
            let last_byte = bytes[expected_bytes - 1];
            let mask = (1u8 << (8 - last_byte_bits)) - 1; // Mask for spare bits
            if (last_byte & mask) != 0 {
                return Err(BitfieldError::NonZeroSpareBits);
            }
        }

        // Check trailing bytes
        if bytes.len() > expected_bytes {
            let extra_bytes = &bytes[expected_bytes..];
            if extra_bytes.iter().any(|&b| b != 0) {
                return Err(BitfieldError::NonZeroSpareBits);
            }
        }

        Ok(Self {
            inner: bytes[..expected_bytes].to_vec(),
            total_pieces: num_pieces,
        })
    }

    pub fn get_inner(&self) -> &[u8] {
        self.inner.as_slice()
    }

    pub fn has_piece(&self, index: usize) -> bool {
        if index >= self.total_pieces {
            return false;
        }
        let byte_index = index / 8;
        let bit_index = 7 - (index % 8);
        (self.inner[byte_index] >> bit_index) & 1 != 0
    }

    pub fn set_piece(&mut self, index: usize) {
        if index >= self.total_pieces {
            return; // Or handle error
        }
        let byte_index = index / 8;
        let bit_index = 7 - (index % 8);
        self.inner[byte_index] |= 1 << bit_index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_new_bitfield() {
        let bitfield = BitField::new(10);
        assert_eq!(bitfield.total_pieces, 10);
        assert_eq!(bitfield.inner.len(), 2); // ceil(10/8) = 2
        assert_eq!(bitfield.inner, vec![0u8, 0u8]);
    }

    #[test]
    fn test_set_and_has_piece() {
        let mut bitfield = BitField::new(10);
        assert!(!bitfield.has_piece(0));
        bitfield.set_piece(0);
        assert!(bitfield.has_piece(0));

        assert!(!bitfield.has_piece(9));
        bitfield.set_piece(9);
        assert!(bitfield.has_piece(9));

        // Test out of bounds
        assert!(!bitfield.has_piece(10));
        bitfield.set_piece(10); // Should not panic and not set
        assert!(!bitfield.has_piece(10));
    }

    #[test]
    fn test_try_from_valid() {
        // 8 pieces, 1 byte
        let bytes = Bytes::from(vec![0b10101010]);
        let bitfield = BitField::try_from(bytes.clone(), 8).unwrap();
        assert_eq!(bitfield.total_pieces, 8);
        assert_eq!(bitfield.inner, vec![0b10101010]);
        assert!(bitfield.has_piece(0));
        assert!(!bitfield.has_piece(1));
        assert!(bitfield.has_piece(2));

        // 10 pieces, 2 bytes, last 6 bits of second byte are spare and should be 0
        let bytes = Bytes::from(vec![0b11111111, 0b11000000]);
        let bitfield = BitField::try_from(bytes.clone(), 10).unwrap();
        assert_eq!(bitfield.total_pieces, 10);
        assert_eq!(bitfield.inner, vec![0b11111111, 0b11000000]);
        assert!(bitfield.has_piece(0));
        assert!(bitfield.has_piece(7));
        assert!(bitfield.has_piece(8));
        assert!(bitfield.has_piece(9));
        assert!(!bitfield.has_piece(10));

        // 10 pieces, 3 bytes, last byte is spare and should be 0
        let bytes_with_trailing_zeros = Bytes::from(vec![0b11111111, 0b11000000, 0u8, 0u8]);
        let bitfield_tz = BitField::try_from(bytes_with_trailing_zeros.clone(), 10).unwrap();
        assert_eq!(bitfield_tz.total_pieces, 10);
        assert_eq!(bitfield_tz.inner, vec![0b11111111, 0b11000000]);
    }

    #[test]
    fn test_try_from_invalid_length_too_short() {
        let bytes = Bytes::from(vec![0b10101010]);
        let result = BitField::try_from(bytes, 10); // Expect 2 bytes
        assert_eq!(
            result.unwrap_err(),
            BitfieldError::InvalidLength {
                expected_len: 2,
                actual_len: 1
            }
        );
    }

    #[test]
    fn test_try_from_non_zero_spare_bits_in_last_byte() {
        // 10 pieces, 2 bytes. Last 6 bits of the second byte should be 0.
        // Here, the 5th bit (from left, 0-indexed) of the second byte is 1 (0b11000100)
        let bytes = Bytes::from(vec![0b11111111, 0b11000100]);
        let result = BitField::try_from(bytes, 10);
        assert_eq!(result.unwrap_err(), BitfieldError::NonZeroSpareBits);
    }

    #[test]
    fn test_try_from_non_zero_spare_bits_in_trailing_bytes() {
        // 8 pieces, 1 byte normally. Provide 2 bytes, second byte is non-zero.
        let bytes = Bytes::from(vec![0b11111111, 0b00000001]);
        let result = BitField::try_from(bytes, 8);
        assert_eq!(result.unwrap_err(), BitfieldError::NonZeroSpareBits);
    }

    #[test]
    fn test_get_inner() {
        let bitfield = BitField::new(5);
        assert_eq!(bitfield.get_inner(), &[0u8]);
        let mut bitfield_2 = BitField::new(12);
        bitfield_2.set_piece(0);
        bitfield_2.set_piece(8);
        // 0th bit is 1 (10000000), 8th bit is 1 (10000000 in second byte)
        assert_eq!(bitfield_2.get_inner(), &[0b10000000, 0b10000000]);
    }
}
