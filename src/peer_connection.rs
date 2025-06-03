use std::{collections::VecDeque, net::SocketAddrV4, os::unix::fs::FileExt};
use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use futures_util::{SinkExt, StreamExt};
use crate::{handshake::{Handshake, Request}, message::{Message, MessageFramer, MessageTag}, piece::Piece, torrent::Torrent};


pub struct PeerConnection<'a> {
    socket: Framed<TcpStream, MessageFramer>,
    torrent: &'a Torrent,
}

impl<'a> PeerConnection<'a> {
    pub async fn new(torrent: &'a Torrent, address: &SocketAddrV4) -> Result<PeerConnection<'a>> {
        let mut stream = TcpStream::
            connect(address)
            .await
            .context("connecting to peer address")?;

        let handshake = Handshake::
            new(torrent)?
            .establish(&mut stream)
            .await?;

        assert_eq!(handshake.length, 19);
        assert_eq!(handshake.bittorrent, *b"BitTorrent protocol");

        println!("Connected to peer: {}", hex::encode(handshake.peer_id));

        Ok(PeerConnection {
            socket: Framed::new(
                stream,
                MessageFramer
            ),
            torrent,
        })
    }

    pub async fn recv_bitfield(&mut self) -> Result<Message> {
        let bitfield = self.socket
            .next()
            .await
            .expect("peer always sends a bitfields")
            .context("peer message was invalid")?;

        assert_eq!(bitfield.tag, MessageTag::Bitfield);

        Ok(bitfield)
    }

    pub async fn send_interested(&mut self) -> Result<()> {
        self.socket
            .send(Message {
                tag: MessageTag::Interested,
                payload: Vec::new(),
            })
            .await
            .context("send interested message")?;

        Ok(())
    }

    pub async fn recv_unchoke(&mut self) -> Result<Message> {
        let unchocke = self.socket
            .next()
            .await
            .expect("peer always sends a unchoke")
            .context("peer message was invalid")?;

        assert_eq!(unchocke.tag, MessageTag::Unchoke);
        assert!(unchocke.payload.is_empty());

        Ok(unchocke)
    }

    pub async fn send_request(&mut self, request: &mut Request) -> Result<()> {
        println!("[send] index: {}; begin: {:05}; length: {}", request.index(), request.begin(), request.length());
        self.socket
            .send(Message {
                tag: MessageTag::Request,
                payload: request.as_bytes_mut().to_vec(),
            })
            .await
            .context(format!("send request"))?;

        Ok(())
    }

    pub async fn recv_piece(&mut self) -> Result<Piece> {
        let piece = self.socket
            .next()
            .await
            .expect("peer always sends a piece")
            .context("peer message was invalid")?;

        assert_eq!(piece.tag, MessageTag::Piece);
        assert!(!piece.payload.is_empty());

        let piece = Piece::from(&piece.payload);
        Ok(piece)
    }

    /// Downloads whole file. Breaks file into pieces and 
    /// each piece into blocks with constant size
    /// 
    /// Requests are pipelined, meaning stream always have N pending requests
    /// for N blocks. After receiving response for block - it is written 
    /// into file at offset that block corresponds to
    /// 
    /// Current implementation N = 5 (always 5 pending requests)
    pub async fn download(&mut self) -> Result<()> {
        let output = self.torrent.output_file()?;
        let (mut pipeline, mut remain) = self.block_requests(5);

        for request in &mut pipeline {
            self.send_request(request).await?;
        }

        while !pipeline.is_empty() {
            let block = self.recv_piece().await?;

            let reqeust_index = pipeline
                .iter()
                .enumerate()
                .find(|(_, a)| a.index() == block.index())
                .map(|(i, _)| i)
                .expect("find request that corresponds to block received");

            match remain.pop_front() {
                None => {
                    pipeline.remove(reqeust_index);
                },
                Some(mut request) => {
                    self.send_request(&mut request).await?;
                    pipeline[reqeust_index] = request;
                }
            }

            let offset = (block.index() * self.torrent.info.piece_length as u32) + block.begin();
            println!("[received] index: {}, begin: {:05}; offset: {:05}; length: {}", block.index(), block.begin(), offset, block.block().len());

            output
                .write_at(&block.block(), offset as u64)
                .context(format!("write block into file at offset {}", offset))?;
        }

        Ok(())
    }

    fn block_requests(&self, split_at: usize) -> (Vec<Request>, VecDeque<Request>) {
        let block_requests = self.torrent
            .pieces_chunked()
            .map(|piece_chunked| piece_chunked.block_requests().collect::<Vec<_>>())            
            .flatten()
            .collect::<Vec<Request>>();

        let pipeline = block_requests[..split_at].to_vec();
        let remain = VecDeque::from(block_requests[split_at..].to_vec());

        (pipeline, remain)
    } 
}