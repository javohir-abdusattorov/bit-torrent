use anyhow::{Context, Ok, Result};
use serde::{Deserialize, Serialize};
pub use peers::Peers;

/// Note: the info_hash field is not included
#[derive(Debug, Clone, Serialize)]
pub struct TrackerRequest {
    /// Info hash of the torrent
    #[serde(skip_serializing)]
    pub info_hash: [u8; 20],

    /// Unique identifier for your client
    pub peer_id: String,

    /// The port your client is listening on
    pub port: u16,

    /// The total amount uploaded so far
    pub uploaded: usize,

    /// The total amount downloaded so far
    pub downloaded: usize,

    /// The number of bytes left to download
    pub left: usize,

    /// Whether the peer list should use the compact representation
    /// For the purposes of this challenge, set this to 1.
    /// The compact representation is more commonly used in the wild, 
    /// the non-compact representation is mostly supported for backward-compatibility.
    pub compact: u8,
}

impl TrackerRequest {
    pub fn url_params(&self, url: &String) -> Result<String> {
        let url_params = serde_urlencoded::to_string(self).context("encode TrackerRequest into URL query params")?;
        let mut tracker_url = reqwest::Url::parse(url).context("parse tracker URL")?;
        tracker_url.set_query(Some(&url_params));

        let tracker_url = format!("{}&info_hash={}", tracker_url, &urlencode(&self.info_hash));
        Ok(tracker_url)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackerResponse {
    /// Indicating how often your client should make a request to the tracker in seconds
    pub interval: usize,

    /// A string, which contains list of peers that your client can connect to.
    /// Each peer is represented using 6 bytes. The first 4 bytes are the peer's IP address and the last 2 bytes are the peer's port number.
    pub peers: Peers,
}

fn urlencode(t: &[u8; 20]) -> String {
    let mut encoded = String::with_capacity(3 * t.len());
    for &byte in t {
        encoded.push('%');
        encoded.push_str(&hex::encode([byte]));
    }

    encoded
}

pub mod peers {
    use core::fmt;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use serde::{de::{self, Visitor}, Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Debug, Clone)]
    pub struct Peers(pub Vec<SocketAddrV4>);
    struct PeersVisitor;
    
    impl<'de> Visitor<'de> for PeersVisitor {
        type Value = Peers;
    
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("6 bytes - first 4 bytes are the peer's IP address and the last 2 bytes are the peer's port number")
        }
    
        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() % 6 != 0 {
                return Err(E::custom(format!("length is {}", v.len())));
            }
    
            Ok(Peers(v
                .chunks_exact(6)
                .map(|slice| {
                    SocketAddrV4::new(
                        Ipv4Addr::new(slice[0], slice[1], slice[2], slice[3]),
                        u16::from_be_bytes([slice[4], slice[5]]),
                    )
                })
                .collect()))
        }
    }
    
    impl<'de> Deserialize<'de> for Peers {
        fn deserialize<D>(deserializer: D) -> Result<Peers, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(PeersVisitor)
        }
    }

    impl Serialize for Peers {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut slice = Vec::with_capacity(6 * self.0.len());
            for peer in &self.0 {
                slice.extend(peer.port().to_be_bytes());
            }
            serializer.serialize_bytes(&slice)
        }
    }

}