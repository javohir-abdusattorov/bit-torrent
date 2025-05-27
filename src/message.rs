use tokio_util::codec::{Decoder, Encoder};
use bytes::{Buf, BufMut, BytesMut};

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageTag {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

impl TryFrom<u8> for MessageTag {
    type Error = std::io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MessageTag::Choke),
            1 => Ok(MessageTag::Unchoke),
            2 => Ok(MessageTag::Interested),
            3 => Ok(MessageTag::NotInterested),
            4 => Ok(MessageTag::Have),
            5 => Ok(MessageTag::Bitfield),
            6 => Ok(MessageTag::Request),
            7 => Ok(MessageTag::Piece),
            8 => Ok(MessageTag::Cancel),
            tag => return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Message tag {} is not supported", tag)
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub tag: MessageTag,
    pub payload: Vec<u8>,
}

const MESSAGE_MAX: usize = 1 << 16;
const MESSAGE_TAG: usize = 1;
const MESSAGE_LENGTH: usize = 4;
const MESSAGE_TAG_AND_LENGTH: usize = MESSAGE_TAG + MESSAGE_LENGTH;

pub struct MessageFramer;

impl Decoder for MessageFramer {
    type Item = Message;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Not enough data to read length marker + tag
        let buffer_len = src.len();
        if buffer_len < MESSAGE_TAG_AND_LENGTH {
            return Ok(None);
        }

        let payload_len = read_u32(&src) as usize;
        let overall_len = MESSAGE_LENGTH + payload_len;
        let contains_payload = buffer_len > MESSAGE_TAG_AND_LENGTH;

        match payload_len {
            // Heartbeat message, discard
            0 => {
                src.advance(MESSAGE_LENGTH);
                self.decode(src)
            },

            // Check that the length is not too large to avoid a denial of
            // service attack where the server runs out of memory.
            payload_len if payload_len > MESSAGE_MAX => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Frame of length {} is too large.", payload_len)
                ))
            },

            // The full string has not yet arrived.
            _ if overall_len > buffer_len => {
                // We reserve more space in the buffer. This is not strictly
                // necessary, but is a good idea performance-wise.
                src.reserve(overall_len - buffer_len);

                Ok(None)
            }

            // Full string arrived, parse tag and payload into `Message`
            _ => {
                let message = Message {
                    tag: MessageTag::try_from(src[MESSAGE_LENGTH])?,
                    payload: if contains_payload { src[MESSAGE_TAG_AND_LENGTH..overall_len].to_vec() } else { Vec::new() },
                };

                // Use advance to modify src such that it no longer contains
                // this frame.
                src.advance(overall_len);

                Ok(Some(message))
            },
        }
    }
}


impl Encoder<Message> for MessageFramer {
    type Error = std::io::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Don't send a Message if it is longer than the other end will
        // accept.
        let buffer_len = item.payload.len() + MESSAGE_TAG;
        if buffer_len > MESSAGE_MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", buffer_len)
            ));
        }

        // Convert the length into a byte array.
        let len_slice = u32::to_be_bytes(buffer_len as u32);

        // Reserve space in the buffer.
        dst.reserve(MESSAGE_TAG_AND_LENGTH + item.payload.len());

        // Write the length and string to the buffer.
        dst.extend_from_slice(&len_slice);
        dst.put_u8(item.tag as u8);
        dst.extend_from_slice(&item.payload);

        Ok(())
    }
}

fn read_u32(bytes: &[u8]) -> u32 {
    let mut length_bytes = [0u8; 4];
    length_bytes.copy_from_slice(&bytes[..4]);
    u32::from_be_bytes(length_bytes)
}