use crate::handshake::Request;

pub const BLOCK_MAX: usize = 1 << 14;


#[repr(C)]
#[derive(Debug)]
pub struct Piece {
    index: [u8; 4],
    begin: [u8; 4],
    block: Vec<u8>,
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

pub struct PieceChunked {
    pub index: usize,
    pub hash: [u8; 20],
    pub size: usize,
    pub number_of_blocks: usize,
}

impl PieceChunked {
    pub fn new(index: usize, hash: [u8; 20], size: usize) -> Self {
        Self {
            index,
            hash,
            size,
            number_of_blocks: (size as f64 / BLOCK_MAX as f64).ceil() as usize,
        }
    }

    pub fn block_requests(&self) -> impl Iterator<Item = Request> + use<'_> {
        (0..self. number_of_blocks)
            .into_iter()
            .map(|index| {
                let begin = index * BLOCK_MAX;
                let last = index == self.number_of_blocks - 1;
                let length = if last {
                    self.size - (index * BLOCK_MAX)
                } else {
                    BLOCK_MAX
                };

                Request::new(
                    self.index as u32,
                    begin as u32,
                    length as u32,
                )
            })
    }
}