use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use serde_bencode;
use hashes::Hashes;
use clap::{Parser, Subcommand};
use sha1::{Sha1, Digest};


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Decode {
        value: String
    },

    Info {
        torrent: PathBuf
    }
}

/// A Metainfo files(also known as .torrent files)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Torrent {
    /// The URL of the tracker
    announce: String,

    info: Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Info {
    /// The suggested name to save the file (or directory) as. It is purely advisory
    /// 
    /// In the single file case, the name key is the name of a file, in the muliple file case, it's the name of a directory
    name: String,

    /// Number of bytes in each piece the file is split into.
    /// 
    /// For the purposes of transfer, files are split into fixed-size pieces which are all the same 
    /// length except for possibly the last one which may be truncated
    #[serde(rename = "piece length")]
    piece_length: usize,

    /// Each entry of pieces is the SHA1 hash of piece at corresponding index
    pieces: Hashes,

    
    #[serde(flatten)]
    keys: Keys,
}

/// There is also a key `length` or a key `files`, but not both or neither. 
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Keys {
    /// If `length` is present then the download represents a single file,
    SingleFile {
        /// Length of the file in bytes
        length: usize
    },

    /// It represents a set of files which go in a directory structure.
    /// For the purposes of the other keys in `Info`, the multi-file case is treated as only 
    /// having a single file by concatenating the files in the order they appear in the files list. 
    MultiFile {
        files: Vec<File>
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct File {
    /// The length of the file, in bytes
    length: usize,

    /// List of UTF-8 encoded strings corresponding to subdirectory names, the last of which is the actual file name
    path: Vec<String>
}


fn main() -> anyhow::Result<()> {
    match Args::parse().command {
        Commands::Decode { value: _value } => {
            // let decoded: serde_json::Value = serde_bencode::from_str(&value)?;
            unimplemented!("serde_bencode -> serde_json::Value doesn't work")
        }

        Commands::Info { torrent } => {
            let file = std::fs::read(torrent).context("read torrent file")?;
            let torrent: Torrent = serde_bencode::from_bytes(&file).context("parse torrent file")?;

            println!("Tracker URL: {}", torrent.announce);
            if let Keys::SingleFile { length } = torrent.info.keys {
                println!("Length: {}", length);
            }

            let info_encoded = serde_bencode::to_bytes(&torrent.info).context("encode info secion")?;
            let mut info_hasher = Sha1::new();
            info_hasher.update(&info_encoded);
            let info_hash = info_hasher.finalize();
            println!("Info hash: {}", hex::encode(&info_hash));
        }
    }

    Ok(())
}


mod hashes {
    use core::fmt;

    use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Debug, Clone)]
    pub struct Hashes(Vec<[u8; 20]>);
    struct HashesVisitor;
    
    impl<'de> Visitor<'de> for HashesVisitor {
        type Value = Hashes;
    
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a byte string whose length is a multiple of 20")
        }
    
        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() % 20 != 0 {
                return Err(E::custom(format!("length is {}", v.len())));
            }
    
            Ok(Hashes(v
                .chunks_exact(20)
                .map(|slice| slice.try_into().expect("guaranteed to be length 20"))
                .collect()))
        }
    }
    
    impl<'de> Deserialize<'de> for Hashes {
        fn deserialize<D>(deserializer: D) -> Result<Hashes, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(HashesVisitor)
        }
    }

    impl Serialize for Hashes {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let slice = self.0.concat();
            serializer.serialize_bytes(&slice)
        }
    }

}