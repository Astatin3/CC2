use std::fmt;

use super::hex_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceHomingAckData {
    Empty,
    UnknownPayload(Vec<u8>),
}

impl DeviceHomingAckData {
    pub fn from_body(body: &[u8]) -> Self {
        if body.is_empty() {
            Self::Empty
        } else {
            Self::UnknownPayload(body.to_vec())
        }
    }
}

impl fmt::Display for DeviceHomingAckData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::UnknownPayload(payload) => {
                write!(f, "UnknownPayload(payload={})", hex_bytes(payload))
            }
        }
    }
}
