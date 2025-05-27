use bittorrent::message::*;
use bittorrent::piece::*;
use bittorrent::torrent::*;
use bittorrent::tracker::*;
use bittorrent::handshake::*;
use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use sha1::{Sha1, Digest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io::{Read, Write};
use std::net::{SocketAddrV4, TcpStream};
use std::path::PathBuf;
use std::str::FromStr;
use clap::{Parser, Subcommand};


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Decode { value: String },

    Info { torrent: PathBuf },

    Peers { torrent: PathBuf },

    Handshake { torrent: PathBuf, peer: String },

    DownloadPiece {
        #[arg(short)]
        output: PathBuf,

        torrent: PathBuf,

        piece: usize,
    },
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Args::parse().command {
        Commands::Decode { value: _value } => {
            // let decoded: serde_json::Value = serde_bencode::from_str(&value)?;
            unimplemented!("serde_bencode -> serde_json::Value doesn't work")
        }

        Commands::Info { torrent } => {
            let torrent = Torrent::try_from(torrent)?;
            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.file_length());

            let (_, info_hash_hex) = torrent.info_hash()?;
            let piece_hashes = torrent.info.pieces.0
                .iter()
                .map(|sha1| format!("\n{}", hex::encode(&sha1)))
                .collect::<String>();

            println!("Info hash: {}", info_hash_hex);
            println!("Piece length: {}", torrent.info.piece_length);
            println!("Piece Hashes: {}", piece_hashes);

        },

        Commands::Peers { torrent } => {
            let torrent = Torrent::try_from(torrent)?;
            let (info_hash_bytes, _) = torrent.info_hash()?;
            let file_length = torrent.file_length();

            let request = TrackerRequest {
                info_hash: info_hash_bytes,
                peer_id: String::from("11111111111111111112"),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: file_length,
                compact: 1,
            };

            let tracker_url = request.url_params(&torrent.announce)?;

            let response = reqwest::blocking::
                get(tracker_url)
                .context("initiate GET request to tracker")?
                .bytes()
                .context("fetch tracker response")
                .map(|bytes| serde_bencode::from_bytes::<TrackerResponse>(&bytes))?
                .context("bencode tracker response")?;

            for peer in &response.peers.0 {
                println!("{peer}");
            }
        },

        Commands::Handshake { torrent, peer } => {
            let torrent = Torrent::try_from(torrent)?;
            let (info_hash_bytes, _) = torrent.info_hash()?;
            let peer = SocketAddrV4::from_str(&peer).context("parse peer address to IPV4")?;
            let mut socket = TcpStream::connect(peer).context("connecting to peer address")?;
            let mut handshake = Handshake::new(info_hash_bytes, *b"11111111111111111112");

            {
                let handshake_bytes = handshake.as_bytes_mut();

                socket
                    .write_all(handshake_bytes)
                    .context("write handshake to socket")?;

                socket
                    .read_exact(handshake_bytes)
                    .context("read handshake from socket")?;
            }

            assert_eq!(handshake.length, 19);
            assert_eq!(handshake.bittorrent, *b"BitTorrent protocol");

            println!("Peer ID: {}", hex::encode(handshake.peer_id));
        },

        Commands::DownloadPiece { output, torrent, piece } => {
            let torrent = Torrent::try_from(torrent)?;
            println!("{torrent:?}");
            let file_length = torrent.file_length();
            let (info_hash_bytes, _) = torrent.info_hash()?;
            let tracker_info = torrent.tracker_info().await?;

            let peer_address = &tracker_info.peers.0[0];
            let mut socket = tokio::net::TcpStream::connect(peer_address).await.context("connecting to peer address")?;
            let mut handshake = Handshake::new(info_hash_bytes, *b"11111111111111111112");

            {
                let handshake_bytes = handshake.as_bytes_mut();

                socket
                    .write_all(handshake_bytes)
                    .await
                    .context("write handshake to socket")?;

                socket
                    .read_exact(handshake_bytes)
                    .await
                    .context("read handshake from socket")?;
            }

            assert_eq!(handshake.length, 19);
            assert_eq!(handshake.bittorrent, *b"BitTorrent protocol");

            println!("Peer ID: {}", hex::encode(handshake.peer_id));

            let mut socket = tokio_util::codec::Framed::new(socket, MessageFramer);
            let bitfield = socket
                .next()
                .await
                .expect("peer always sends a bitfields")
                .context("peer message was invalid")?;
            assert_eq!(bitfield.tag, MessageTag::Bitfield);

            socket
                .send(Message {
                    tag: MessageTag::Interested,
                    payload: Vec::new(),
                })
                .await
                .context("send interested message")?;

            let unchocke = socket
                .next()
                .await
                .expect("peer always sends a unchoke")
                .context("peer message was invalid")?;
            assert_eq!(unchocke.tag, MessageTag::Unchoke);
            assert!(unchocke.payload.is_empty());

            let piece_hash = torrent.info.pieces.0[piece];
            let piece_size = if piece == torrent.info.pieces.0.len() - 1 {
                file_length - (piece * torrent.info.piece_length)
            } else {
                torrent.info.piece_length
            };
            let number_of_blocks = (piece_size as f64 / BLOCK_MAX as f64).ceil() as usize;
            let mut all_blocks = Vec::with_capacity(piece_size);

            for block_index in 0..number_of_blocks {
                let block_length = if block_index == number_of_blocks - 1 {
                    piece_size - (block_index * BLOCK_MAX)
                } else {
                    BLOCK_MAX
                };

                let begin = block_index * BLOCK_MAX;
                let mut request = Request::new(
                    piece as u32,
                    begin as u32,
                    block_length as u32
                );

                let request_bytes = Vec::from(request.as_bytes_mut());
                socket
                    .send(Message {
                        tag: MessageTag::Request,
                        payload: request_bytes,
                    })
                    .await
                    .context(format!("send request for block {block_index}"))?;

                let piece_message = socket
                    .next()
                    .await
                    .expect("peer always sends a piece")
                    .context("peer message was invalid")?;
                assert_eq!(piece_message.tag, MessageTag::Piece);
                assert!(!piece_message.payload.is_empty());

                let piece = Piece::from(&piece_message.payload);
                all_blocks.extend(piece.block());
            }

            assert_eq!(piece_size, all_blocks.len());

            let mut hasher = Sha1::new();
            hasher.update(&all_blocks);
            let hash: [u8; 20] = hasher
                .finalize()
                .try_into()
                .expect("GenericArray<_, 20> == [_; 20]");
            assert_eq!(hash, piece_hash);

            std::fs::write(output, all_blocks)?;
        }
    }

    Ok(())
}