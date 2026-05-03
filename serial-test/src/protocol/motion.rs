use std::fmt;

use super::hex_bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionChannel {
    Channel0d,
    Channel10,
    Channel13,
    Unknown(u8),
}

impl MotionChannel {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x0d => Self::Channel0d,
            0x10 => Self::Channel10,
            0x13 => Self::Channel13,
            _ => Self::Unknown(byte),
        }
    }
}

impl fmt::Display for MotionChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel0d => f.write_str("Channel0d"),
            Self::Channel10 => f.write_str("Channel10"),
            Self::Channel13 => f.write_str("Channel13"),
            Self::Unknown(byte) => write!(f, "Unknown(0x{byte:02x})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionChunk {
    pub channel: MotionChannel,
    pub encoded_point: Vec<u8>,
}

impl fmt::Display for MotionChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{channel={}, encoded_point={}}}",
            self.channel,
            hex_bytes(&self.encoded_point)
        )
    }
}

pub fn parse_motion_chunks(body: &[u8]) -> Option<Vec<MotionChunk>> {
    if body.is_empty() {
        return None;
    }

    let mut chunks = Vec::new();
    let mut offset = 0;

    loop {
        let channel = *body.get(offset)?;
        let point_start = offset + 1;
        let mut next = body.len();

        for i in point_start..body.len().saturating_sub(1) {
            if body[i] == 0x17 {
                next = i;
                break;
            }
        }

        chunks.push(MotionChunk {
            channel: MotionChannel::from_byte(channel),
            encoded_point: body[point_start..next].to_vec(),
        });

        if next == body.len() {
            break;
        }

        offset = next + 1;
        if offset >= body.len() {
            return None;
        }
    }

    Some(chunks)
}

pub fn format_chunks(chunks: &[MotionChunk]) -> String {
    chunks
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
