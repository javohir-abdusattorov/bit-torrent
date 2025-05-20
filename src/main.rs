use bittorrent::handshake::*;
use bittorrent::torrent::*;
use bittorrent::tracker::*;
use anyhow::Context;
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

    Handshake { torrent: PathBuf, peer: String }
}


fn main() -> anyhow::Result<()> {
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
            let file_length = torrent.file_length();

            let (info_hash_bytes, _) = torrent.info_hash()?;

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
                // Safety: Handshake is a POD with rep(C)
                let handshake_bytes = &mut handshake as *mut Handshake as *mut [u8; std::mem::size_of::<Handshake>()];
                let handshake_bytes: &mut [u8; std::mem::size_of::<Handshake>()] = unsafe { &mut *handshake_bytes };

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
        }
    }

    Ok(())
}