use std::path::PathBuf;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Sha1, Digest};
pub use hashes::Hashes;
use crate::tracker::{TrackerRequest, TrackerResponse};


/// A Metainfo files(also known as .torrent files)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Torrent {
    /// The URL of the tracker
    pub announce: String,

    pub info: Info,
}

impl Torrent {
    pub fn peer_id(&self) -> [u8; 20] {
        *b"12345611111111111112"
    }

    pub fn info_hash(&self) -> Result<[u8; 20]> {
        let encoded = serde_bencode::to_bytes(&self.info).context("encode info secion")?;
        let mut hasher = Sha1::new();
        hasher.update(&encoded);
        let bytes = hasher.finalize();
        let array = bytes[..].try_into().expect("GenericArray<_, 20> == [_; 20]");

        Ok(array)
    }

    pub fn file_length(&self) -> usize {
        match &self.info.keys {
            Keys::SingleFile { length } => *length,
            Keys::MultiFile { files: _ } => 0,
        }
    }

    pub async fn tracker_info(&self) -> Result<TrackerResponse> {
        let request = self.tracker_request()?;

        let tracker_url = request.url_params(&self.announce)?;
        let tracker_info = reqwest::
            get(tracker_url)
            .await
            .context("initiate GET request to tracker")?
            .bytes()
            .await
            .context("fetch tracker response")
            .map(|bytes| serde_bencode::from_bytes::<TrackerResponse>(&bytes))?
            .context("bencode tracker response")?;

        Ok(tracker_info)
    }

    fn tracker_request(&self) -> Result<TrackerRequest> {
        let info_hash_bytes = self.info_hash()?;
        let file_length = self.file_length();

        Ok(TrackerRequest {
            info_hash: info_hash_bytes,
            peer_id: String::from_utf8(self.peer_id().to_vec())?,
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: file_length,
            compact: 1,
        })
    }
}

impl TryFrom<PathBuf> for Torrent {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        let file = std::fs::read(path).context("read torrent file")?;
        Ok(serde_bencode::from_bytes(&file).context("parse torrent file")?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    /// The suggested name to save the file (or directory) as. It is purely advisory
    /// 
    /// In the single file case, the name key is the name of a file, in the muliple file case, it's the name of a directory
    pub name: String,

    /// Number of bytes in each piece the file is split into.
    /// 
    /// For the purposes of transfer, files are split into fixed-size pieces which are all the same 
    /// length except for possibly the last one which may be truncated
    #[serde(rename = "piece length")]
    pub piece_length: usize,

    /// Each entry of pieces is the SHA1 hash of piece at corresponding index
    pub pieces: Hashes,

    #[serde(flatten)]
    pub keys: Keys,
}

/// There is also a key `length` or a key `files`, but not both or neither. 
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Keys {
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
pub struct File {
    /// The length of the file, in bytes
    length: usize,

    /// List of UTF-8 encoded strings corresponding to subdirectory names, the last of which is the actual file name
    path: Vec<String>
}

pub mod hashes {
    use core::fmt;
    use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Debug, Clone)]
    pub struct Hashes(pub Vec<[u8; 20]>);
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