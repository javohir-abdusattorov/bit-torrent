use bittorrent::peer_connection::PeerConnection;
use bittorrent::torrent::*;
use anyhow::Context;
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

    DownloadPiece { torrent: PathBuf },
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

        Commands::DownloadPiece { torrent } => {
            let torrent = Torrent::try_from(torrent)?;
            let tracker_info = torrent.tracker_info().await?;
            let mut peer = PeerConnection::new(&torrent, &tracker_info.peers.0[0]).await?;

            peer.recv_bitfield().await?;
            peer.send_interested().await?;
            peer.recv_unchoke().await?;
            peer.download().await?;
        }
    }

    Ok(())
}