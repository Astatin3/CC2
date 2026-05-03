use std::fmt;

use super::hex_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePeriodicStatusData {
    PeriodicStatus {
        group: u8,
        fields: Vec<u8>,
        status_tail: Vec<u8>,
    },
    UnknownPayload(Vec<u8>),
}

impl DevicePeriodicStatusData {
    pub fn from_body(body: &[u8]) -> Self {
        let Some((&group, fields)) = body.split_first() else {
            return Self::UnknownPayload(body.to_vec());
        };
        let status_tail = if fields.len() >= 2 {
            fields[fields.len() - 2..].to_vec()
        } else {
            fields.to_vec()
        };

        Self::PeriodicStatus {
            group,
            fields: fields.to_vec(),
            status_tail,
        }
    }
}

impl fmt::Display for DevicePeriodicStatusData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodicStatus {
                group,
                fields,
                status_tail,
            } => write!(
                f,
                "PeriodicStatus(group=0x{group:02x}, status_tail={}, fields={})",
                hex_bytes(status_tail),
                hex_bytes(fields)
            ),
            Self::UnknownPayload(payload) => {
                write!(f, "UnknownPayload(payload={})", hex_bytes(payload))
            }
        }
    }
}
