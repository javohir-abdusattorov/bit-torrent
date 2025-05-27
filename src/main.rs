use bittorrent::peer_connection::PeerConnection;
use bittorrent::piece::*;
use bittorrent::torrent::*;
use bittorrent::handshake::*;
use anyhow::Context;
use sha1::{Sha1, Digest};
use std::net::SocketAddrV4;
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

            let piece_hashes = torrent.info.pieces.0
                .iter()
                .map(|sha1| format!("\n{}", hex::encode(&sha1)))
                .collect::<String>();

            println!("Info hash: {}", hex::encode(torrent.info_hash()?));
            println!("Piece length: {}", torrent.info.piece_length);
            println!("Piece Hashes: {}", piece_hashes);

        },

        Commands::Peers { torrent } => {
            let torrent = Torrent::try_from(torrent)?;
            let response = torrent.tracker_info().await?;

            for peer in &response.peers.0 {
                println!("{peer}");
            }
        },

        Commands::Handshake { torrent, peer } => {
            let torrent = Torrent::try_from(torrent)?;

            let peer_address = SocketAddrV4::from_str(&peer).context("parse peer address to IPV4")?;
            PeerConnection::new(&torrent, &peer_address).await?;
        },

        Commands::DownloadPiece { output, torrent, piece } => {
            let torrent = Torrent::try_from(torrent)?;
            println!("{torrent:?}");

            let tracker_info = torrent.tracker_info().await?;
            let mut peer = PeerConnection::new(&torrent, &tracker_info.peers.0[0]).await?;

            peer.recv_bitfield().await?;
            peer.send_interested().await?;
            peer.recv_unchoke().await?;

            let file_length = torrent.file_length();
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
                peer.send_request(request_bytes).await?;

                let piece = peer.recv_piece().await?;
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