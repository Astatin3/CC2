mod device_action_response;
mod device_homing_ack;
mod device_homing_status;
mod device_periodic_status;
mod device_query_response;
mod device_sensor_status;
mod host_action;
mod host_control_response;
mod host_motion_control;
mod host_motion_program;
mod host_periodic_query;
pub mod motion;

use std::fmt;

pub use device_action_response::DeviceActionResponseData;
pub use device_homing_ack::DeviceHomingAckData;
pub use device_homing_status::DeviceHomingStatusData;
pub use device_periodic_status::DevicePeriodicStatusData;
pub use device_query_response::DeviceQueryResponseData;
pub use device_sensor_status::DeviceSensorStatusData;
pub use host_action::HostActionData;
pub use host_control_response::HostControlResponseData;
pub use host_motion_control::HostMotionControlData;
pub use host_motion_program::HostMotionProgramData;
pub use host_periodic_query::HostPeriodicQueryData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    HostWrite,
    DeviceRead,
    Ioctl,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostWrite => f.write_str("host->mcu"),
            Self::DeviceRead => f.write_str("mcu->host"),
            Self::Ioctl => f.write_str("ioctl"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McuCommand {
    HostPeriodicQuery(HostPeriodicQueryData),
    HostMotionControl(HostMotionControlData),
    HostMotionProgram(HostMotionProgramData),
    HostControlResponse(HostControlResponseData),
    HostAction(HostActionData),
    DeviceQueryResponse(DeviceQueryResponseData),
    DeviceHomingAck(DeviceHomingAckData),
    DeviceHomingStatus(DeviceHomingStatusData),
    DevicePeriodicStatus(DevicePeriodicStatusData),
    DeviceActionResponse(DeviceActionResponseData),
    DeviceSensorStatus(DeviceSensorStatusData),
    TransportAck(TransportAckData),
    Unknown(u8),
    MalformedFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportAckData {
    pub crc: Vec<u8>,
}

impl McuCommand {
    fn from_parts(direction: Direction, message_id: u8, body: &[u8]) -> Self {
        match (direction, message_id) {
            (Direction::HostWrite, 0x05) => {
                Self::HostPeriodicQuery(HostPeriodicQueryData::from_body(body))
            }
            (Direction::HostWrite, 0x17) => {
                Self::HostMotionControl(HostMotionControlData::from_body(body))
            }
            (Direction::HostWrite, 0x18) => {
                Self::HostMotionProgram(HostMotionProgramData::from_body(body))
            }
            (Direction::HostWrite, 0x21) => {
                Self::HostControlResponse(HostControlResponseData::from_body(body))
            }
            (Direction::HostWrite, 0x2a) => Self::HostAction(HostActionData::from_body(body)),
            (Direction::DeviceRead, 0x3b) => {
                Self::DeviceQueryResponse(DeviceQueryResponseData::from_body(body))
            }
            (Direction::DeviceRead, 0x31) => {
                Self::DeviceHomingAck(DeviceHomingAckData::from_body(body))
            }
            (Direction::DeviceRead, 0x42) => {
                Self::DeviceHomingStatus(DeviceHomingStatusData::from_body(body))
            }
            (Direction::DeviceRead, 0x43) => {
                Self::DevicePeriodicStatus(DevicePeriodicStatusData::from_body(body))
            }
            (Direction::DeviceRead, 0x46) => {
                Self::DeviceActionResponse(DeviceActionResponseData::from_body(body))
            }
            (Direction::DeviceRead, 0x48) => {
                Self::DeviceSensorStatus(DeviceSensorStatusData::from_body(body))
            }
            _ => Self::Unknown(message_id),
        }
    }
}

impl fmt::Display for McuCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostPeriodicQuery(_) => f.write_str("HostPeriodicQuery"),
            Self::HostMotionControl(_) => f.write_str("HostMotionControl"),
            Self::HostMotionProgram(_) => f.write_str("HostMotionProgram"),
            Self::HostControlResponse(_) => f.write_str("HostControlResponse"),
            Self::HostAction(_) => f.write_str("HostAction"),
            Self::DeviceQueryResponse(_) => f.write_str("DeviceQueryResponse"),
            Self::DeviceHomingAck(_) => f.write_str("DeviceHomingAck"),
            Self::DeviceHomingStatus(_) => f.write_str("DeviceHomingStatus"),
            Self::DevicePeriodicStatus(_) => f.write_str("DevicePeriodicStatus"),
            Self::DeviceActionResponse(_) => f.write_str("DeviceActionResponse"),
            Self::DeviceSensorStatus(_) => f.write_str("DeviceSensorStatus"),
            Self::TransportAck(_) => f.write_str("TransportAck"),
            Self::Unknown(opcode) => write!(f, "Unknown(0x{opcode:02x})"),
            Self::MalformedFrame => f.write_str("MalformedFrame"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McuFrame {
    pub len: u8,
    pub seq: u8,
    pub message_id: Option<u8>,
    pub command: McuCommand,
    pub messages: Vec<KlipperMessage>,
    pub content: Vec<u8>,
    pub body: Vec<u8>,
    pub trailer: Vec<u8>,
    pub computed_crc: Vec<u8>,
    pub crc_valid: bool,
    pub terminator: u8,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlipperMessage {
    pub message_id: u8,
    pub payload: Vec<u8>,
}

impl McuFrame {
    pub fn parse(direction: Direction, bytes: &[u8]) -> Self {
        if bytes.len() < 4
            || bytes.first().copied() != Some(bytes.len() as u8)
            || bytes.last().copied() != Some(0x7e)
        {
            return Self {
                len: bytes.first().copied().unwrap_or(0),
                seq: bytes.get(1).copied().unwrap_or(0),
                message_id: bytes.get(2).copied(),
                command: McuCommand::MalformedFrame,
                messages: Vec::new(),
                content: Vec::new(),
                body: Vec::new(),
                trailer: Vec::new(),
                computed_crc: Vec::new(),
                crc_valid: false,
                terminator: bytes.last().copied().unwrap_or(0),
                raw: bytes.to_vec(),
            };
        }

        let len = bytes[0];
        let seq = bytes[1];
        let trailer_len = 2;
        let body_end = bytes.len() - trailer_len - 1;
        let content = bytes[2..body_end].to_vec();
        let trailer = bytes[body_end..bytes.len() - 1].to_vec();
        let computed_crc = crc16_ccitt_klipper(&bytes[..body_end])
            .to_be_bytes()
            .to_vec();
        let crc_valid = trailer == computed_crc;
        let messages = parse_klipper_messages(&content);
        let message_id = messages.first().map(|message| message.message_id);
        let body = if content.len() > 1 {
            content[1..].to_vec()
        } else {
            Vec::new()
        };
        let command = if body_end == 2 {
            McuCommand::TransportAck(TransportAckData {
                crc: trailer.clone(),
            })
        } else if let Some(message_id) = message_id {
            McuCommand::from_parts(direction, message_id, &body)
        } else {
            McuCommand::MalformedFrame
        };

        Self {
            len,
            seq,
            message_id,
            command,
            messages,
            content,
            body,
            trailer,
            computed_crc,
            crc_valid,
            terminator: 0x7e,
            raw: bytes.to_vec(),
        }
    }
}

pub fn split_frames(bytes: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    let mut offset = 0;

    while offset < bytes.len() {
        let Some(&len) = bytes.get(offset) else {
            break;
        };
        let len = len as usize;

        if len == 0 || offset + len > bytes.len() {
            frames.push(&bytes[offset..]);
            break;
        }

        frames.push(&bytes[offset..offset + len]);
        offset += len;
    }

    frames
}

pub fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn crc16_ccitt_klipper(bytes: &[u8]) -> u16 {
    let mut crc = 0xffffu16;

    for byte in bytes {
        crc ^= *byte as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x8408;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

pub fn parse_klipper_messages(content: &[u8]) -> Vec<KlipperMessage> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut messages = Vec::new();
    let mut start = 0;

    for index in 1..content.len() {
        if is_observed_message_id(content[index]) {
            messages.push(KlipperMessage {
                message_id: content[start],
                payload: content[start + 1..index].to_vec(),
            });
            start = index;
        }
    }

    messages.push(KlipperMessage {
        message_id: content[start],
        payload: content[start + 1..].to_vec(),
    });

    messages
}

fn is_observed_message_id(byte: u8) -> bool {
    matches!(
        byte,
        0x17 | 0x18
            | 0x19
            | 0x1a
            | 0x1d
            | 0x1e
            | 0x20
            | 0x21
            | 0x22
            | 0x28
            | 0x2a
            | 0x3b
            | 0x3d
            | 0x40
            | 0x41
            | 0x42
            | 0x43
            | 0x46
            | 0x48
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_periodic_query_frame() {
        let bytes = [0x06, 0x14, 0x05, 0x4a, 0xb6, 0x7e];
        let frame = McuFrame::parse(Direction::HostWrite, &bytes);

        assert_eq!(
            frame.command,
            McuCommand::HostPeriodicQuery(HostPeriodicQueryData::Empty)
        );
        assert_eq!(frame.seq, 0x14);
        assert_eq!(frame.message_id, Some(0x05));
        assert_eq!(frame.trailer, [0x4a, 0xb6]);
        assert!(frame.crc_valid);
    }

    #[test]
    fn splits_aggregated_read_frames() {
        let bytes = [
            0x0b, 0x15, 0x3b, 0x82, 0x9f, 0xb7, 0xbb, 0x5d, 0xe5, 0x75, 0x7e, 0x05, 0x15, 0xc9,
            0x2c, 0x7e,
        ];
        let frames = split_frames(&bytes);

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].len(), 11);
        assert_eq!(frames[1].len(), 5);
        let ack = McuFrame::parse(Direction::DeviceRead, frames[1]);
        assert_eq!(
            ack.command,
            McuCommand::TransportAck(TransportAckData {
                crc: vec![0xc9, 0x2c]
            })
        );
        assert_eq!(ack.message_id, None);
        assert!(ack.crc_valid);
    }

    #[test]
    fn validates_klipper_crc_vectors() {
        assert_eq!(crc16_ccitt_klipper(&[0x05, 0x10]), 0x9e81);
        assert_eq!(crc16_ccitt_klipper(&[0x05, 0x1f]), 0x6676);
        assert_eq!(crc16_ccitt_klipper(&[0x06, 0x14, 0x05]), 0x4ab6);
    }

    #[test]
    fn extracts_observed_encoded_messages() {
        let content = [
            0x2a, 0x0c, 0x05, 0xea, 0x03, 0xe8, 0xad, 0xe1, 0x0a, 0x17, 0x10,
        ];
        let messages = parse_klipper_messages(&content);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].message_id, 0x2a);
        assert_eq!(messages[1].message_id, 0x17);
    }

    #[test]
    fn host_motion_program_decodes_opcode_18() {
        let bytes = [
            0x3a, 0x10, 0x18, 0x13, 0x00, 0x17, 0x13, 0x80, 0xf0, 0xe0, 0x82, 0x65, 0x01, 0x00,
            0x17, 0x13, 0x83, 0xb0, 0x5c, 0x02, 0xff, 0x8f, 0x3a, 0x17, 0x13, 0x82, 0x81, 0x71,
            0x03, 0xe6, 0x2e, 0x17, 0x13, 0x81, 0xb7, 0x5d, 0x06, 0xf6, 0x75, 0x17, 0x13, 0x81,
            0x83, 0x73, 0x0a, 0xfc, 0x39, 0x17, 0x13, 0x80, 0xe2, 0x1d, 0x10, 0xfe, 0x3c, 0x53,
            0x10, 0x7e,
        ];
        let frame = McuFrame::parse(Direction::HostWrite, &bytes);

        assert!(matches!(frame.command, McuCommand::HostMotionProgram(_)));
        assert_eq!(frame.body[0], 0x13);
        assert_eq!(frame.trailer, [0x53, 0x10]);
        assert!(frame.crc_valid);
    }
}
