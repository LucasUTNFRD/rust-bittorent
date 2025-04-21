use std::fmt;

use hex::FromHexError;
use thiserror::Error;

pub struct PeerId(pub [u8; 20]);
pub struct PieceHash(pub [u8; 20]);

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PieceHashError {
    #[error("Invalid Lenght")]
    InvalidLenght,
}

impl TryFrom<&[u8]> for PieceHash {
    type Error = PieceHashError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() == 20 {
            let mut bytes = [0u8; 20];
            bytes.copy_from_slice(value);
            Ok(PieceHash(bytes))
        } else {
            Err(PieceHashError::InvalidLenght)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InfoHash(pub [u8; 20]);

impl fmt::Debug for InfoHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use internal helper or hex crate if feature enabled
        let hex_string = self.to_hex();
        f.debug_tuple("InfoHash").field(&hex_string).finish()
    }
}

impl fmt::Display for InfoHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use internal helper or hex crate if feature enabled
        let hex_string = self.to_hex();
        write!(f, "{}", hex_string)
    }
}

impl From<[u8; 20]> for InfoHash {
    fn from(bytes: [u8; 20]) -> Self {
        InfoHash(bytes)
    }
}

#[derive(Debug, Error)]
pub enum InfoHashError {
    #[error("Invalid hash length{0}")]
    InvalidHashLength(usize),
    #[error("Invalid hex length{0}")]
    InvalidHexLength(usize),
    #[error("Hex error {0}")]
    InvalidHexEncoding(FromHexError),
}

impl TryFrom<&[u8]> for InfoHash {
    type Error = InfoHashError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() == 20 {
            let mut bytes = [0u8; 20];
            bytes.copy_from_slice(value);
            Ok(InfoHash(bytes))
        } else {
            Err(InfoHashError::InvalidHashLength(value.len()))
        }
    }
}

impl InfoHash {
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex_str: &str) -> Result<Self, InfoHashError> {
        if hex_str.len() != 40 {
            return Err(InfoHashError::InvalidHexLength(hex_str.len())); // Add variant
        }
        let mut bytes = [0u8; 20];
        hex::decode_to_slice(hex_str, &mut bytes).map_err(InfoHashError::InvalidHexEncoding)?; // Add variant
        Ok(InfoHash(bytes))
    }

    pub fn as_bytes(&self) -> [u8; 20] {
        self.0
    }
}
