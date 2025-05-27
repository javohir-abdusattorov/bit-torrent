pub const BLOCK_MAX: usize = 1 << 14;


#[repr(C)]
#[derive(Debug)]
pub struct Piece {
    pub index: [u8; 4],
    pub begin: [u8; 4],
    pub block: Vec<u8>,
}

impl From<&Vec<u8>> for Piece {
    fn from(value: &Vec<u8>) -> Self {
        Piece {
            index: value[..4].try_into().expect("always 4 bytes"),
            begin: value[4..8].try_into().expect("always 4 bytes"),
            block: value[8..].to_vec(),
        }
    }
}

impl Piece {
    pub fn index(&self) -> u32 {
        u32::from_be_bytes(self.index)
    }

    pub fn begin(&self) -> u32 {
        u32::from_be_bytes(self.begin)
    }

    pub fn block(&self) -> &Vec<u8> {
        &self.block
    }
}