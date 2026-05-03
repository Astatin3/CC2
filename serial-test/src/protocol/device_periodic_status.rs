use std::fmt;

use super::hex_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePeriodicStatusData {
    UnknownPayload(Vec<u8>),
}

impl DevicePeriodicStatusData {
    pub fn from_body(body: &[u8]) -> Self {
        Self::UnknownPayload(body.to_vec())
    }
}

impl fmt::Display for DevicePeriodicStatusData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPayload(payload) => {
                write!(f, "UnknownPayload(payload={})", hex_bytes(payload))
            }
        }
    }
}
