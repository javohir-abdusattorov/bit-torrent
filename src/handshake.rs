use anyhow::{Context, Result};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use crate::torrent::Torrent;


#[repr(C)]
#[derive(Debug, Clone)]
pub struct Handshake {
    pub length: u8,
    pub bittorrent: [u8; 19],
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(torrent: &Torrent) -> Result<Self> {
        Ok(Self {
            length: 19,
            bittorrent: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash: torrent.info_hash()?,
            peer_id: torrent.peer_id(),
        })
    }

    pub async fn establish(mut self, socket: &mut TcpStream) -> Result<Self> {
        let handshake_bytes = self.as_bytes_mut();

        socket
            .write_all(handshake_bytes)
            .await
            .context("write handshake to socket")?;

        socket
            .read_exact(handshake_bytes)
            .await
            .context("read handshake from socket")?;

        Ok(self)
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let bytes = self as *mut Self as *mut [u8; std::mem::size_of::<Self>()];
        let bytes: &mut [u8; std::mem::size_of::<Self>()] = unsafe { &mut *bytes };
        bytes
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Request {
    index: [u8; 4],
    begin: [u8; 4],
    length: [u8; 4],
}

impl Request {
    pub fn new(index: u32, begin: u32, length: u32) -> Self {
        Self {
            index: index.to_be_bytes(),
            begin: begin.to_be_bytes(),
            length: length.to_be_bytes(),
        }
    }

    pub fn index(&self) -> u32 {
        u32::from_be_bytes(self.index)
    }

    pub fn begin(&self) -> u32 {
        u32::from_be_bytes(self.begin)
    }

    pub fn length(&self) -> u32 {
        u32::from_be_bytes(self.length)
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        let bytes = self as *mut Self as *mut [u8; std::mem::size_of::<Self>()];
        unsafe { &mut *bytes }
    }
}