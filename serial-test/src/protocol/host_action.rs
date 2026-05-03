use std::fmt;

use super::hex_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostActionData {
    ShortAction { group: u8, value: u8, arg: u16 },
    ExtendedAction { fields: Vec<u8> },
    UnknownPayload(Vec<u8>),
}

impl HostActionData {
    pub fn from_body(body: &[u8]) -> Self {
        if body.len() == 8 && body[..3] == [0x0c, 0x05, 0xea] && body[7] == 0x0a {
            return Self::ShortAction {
                group: body[3],
                value: body[4],
                arg: u16::from_be_bytes([body[5], body[6]]),
            };
        }

        if body.len() == 13 && body[..3] == [0x0c, 0x0a, 0xea] {
            return Self::ExtendedAction {
                fields: body[3..].to_vec(),
            };
        }

        Self::UnknownPayload(body.to_vec())
    }
}

impl fmt::Display for HostActionData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortAction { group, value, arg } => write!(
                f,
                "ShortAction(group=0x{group:02x}, value=0x{value:02x}, arg=0x{arg:04x})"
            ),
            Self::ExtendedAction { fields } => {
                write!(f, "ExtendedAction(fields={})", hex_bytes(fields))
            }
            Self::UnknownPayload(payload) => {
                write!(f, "UnknownPayload(payload={})", hex_bytes(payload))
            }
        }
    }
}
