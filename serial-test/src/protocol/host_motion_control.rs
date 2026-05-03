use std::fmt;

use super::hex_bytes;
use super::motion::{MotionChunk, format_chunks, parse_motion_chunks};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostMotionControlData {
    MotionScript { chunks: Vec<MotionChunk> },
    UnknownPayload(Vec<u8>),
}

impl HostMotionControlData {
    pub fn from_body(body: &[u8]) -> Self {
        if let Some(chunks) = parse_motion_chunks(body) {
            Self::MotionScript { chunks }
        } else {
            Self::UnknownPayload(body.to_vec())
        }
    }
}

impl fmt::Display for HostMotionControlData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MotionScript { chunks } => {
                write!(f, "MotionScript(chunks=[{}])", format_chunks(chunks))
            }
            Self::UnknownPayload(payload) => {
                write!(f, "UnknownPayload(payload={})", hex_bytes(payload))
            }
        }
    }
}
