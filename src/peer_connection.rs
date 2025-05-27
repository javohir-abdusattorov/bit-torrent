use std::net::SocketAddrV4;
use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use futures_util::{SinkExt, StreamExt};
use crate::{handshake::Handshake, message::{Message, MessageFramer, MessageTag}, piece::Piece, torrent::Torrent};


pub struct PeerConnection {
    socket: Framed<TcpStream, MessageFramer>,
}

impl PeerConnection {
    pub async fn new(torrent: &Torrent, address: &SocketAddrV4) -> Result<PeerConnection> {
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

    pub async fn send_request(&mut self, request_bytes: Vec<u8>) -> Result<()> {
        self.socket
            .send(Message {
                tag: MessageTag::Request,
                payload: request_bytes,
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
}