// ---- BITFIELD -----

#[derive(Debug, Clone)]
pub struct Bitfield {
    total_pieces: usize,
    inner: Vec<u8>,
}

// #[derive(Debug, Error)]
// pub enum BitfieldError {
//     #[error("Invalid data")]
//     InvalidData,
// }

impl Bitfield {
    pub fn new(total_pieces: usize) -> Self {
        let num_bytes = (total_pieces + 7) / 8;
        Bitfield {
            total_pieces,
            inner: vec![0u8; num_bytes],
        }
    }

    pub fn get_inner(&self) -> &[u8] {
        self.inner.as_slice()
    }

    pub fn has_piece<I: Into<usize>>(&self, index: I) -> bool {
        let i = index.into();
        if i >= self.total_pieces {
            return false;
        }
        let byte = i / 8;
        let bit = 7 - (i % 8);
        self.inner[byte] & (1 << bit) != 0
    }

    pub fn set_piece<I: Into<usize>>(&mut self, index: I) {
        let i = index.into();
        if i >= self.total_pieces {
            return;
        }
        let byte = i / 8;
        let bit = 7 - (i % 8);
        self.inner[byte] |= 1 << bit;
    }
}
