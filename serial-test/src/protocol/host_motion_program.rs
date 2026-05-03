use std::fmt;

use super::hex_bytes;
use super::motion::{MotionChannel, MotionChunk, format_chunks, parse_motion_chunks};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostMotionProgramData {
    StartMotionProgram {
        channel: MotionChannel,
        flags: u8,
        chunks: Vec<MotionChunk>,
        raw_tail: Vec<u8>,
    },
    UnknownPayload(Vec<u8>),
}

impl HostMotionProgramData {
    pub fn from_body(body: &[u8]) -> Self {
        let Some((&channel, rest)) = body.split_first() else {
            return Self::UnknownPayload(body.to_vec());
        };
        let Some((&flags, rest)) = rest.split_first() else {
            return Self::UnknownPayload(body.to_vec());
        };

        let chunks = if rest.first().copied() == Some(0x17) {
            parse_motion_chunks(&rest[1..]).unwrap_or_default()
        } else {
            parse_motion_chunks(rest).unwrap_or_default()
        };

        Self::StartMotionProgram {
            channel: MotionChannel::from_byte(channel),
            flags,
            chunks,
            raw_tail: Vec::new(),
        }
    }
}

impl fmt::Display for HostMotionProgramData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartMotionProgram {
                channel,
                flags,
                chunks,
                raw_tail,
            } => write!(
                f,
                "StartMotionProgram(channel={channel}, flags=0x{flags:02x}, chunks=[{}], raw_tail={})",
                format_chunks(chunks),
                hex_bytes(raw_tail)
            ),
            Self::UnknownPayload(payload) => {
                write!(f, "UnknownPayload(payload={})", hex_bytes(payload))
            }
        }
    }
}
