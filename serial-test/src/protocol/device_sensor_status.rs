use std::fmt;

use super::hex_bytes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSensorStatusData {
    SensorStatus {
        sensor_id: u8,
        extruder_group: Option<ExtruderTelemetryGroup>,
        fields: Vec<u8>,
    },
    UnknownPayload(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtruderTelemetryGroup {
    ExtruderDown,
    ExtruderUp,
}

impl fmt::Display for ExtruderTelemetryGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtruderDown => f.write_str("ExtruderDown(0x96)"),
            Self::ExtruderUp => f.write_str("ExtruderUp(0x97)"),
        }
    }
}

impl DeviceSensorStatusData {
    pub fn from_body(body: &[u8]) -> Self {
        let Some((&sensor_id, fields)) = body.split_first() else {
            return Self::UnknownPayload(body.to_vec());
        };

        let extruder_group = fields.iter().find_map(|byte| match byte {
            0x96 => Some(ExtruderTelemetryGroup::ExtruderDown),
            0x97 => Some(ExtruderTelemetryGroup::ExtruderUp),
            _ => None,
        });

        Self::SensorStatus {
            sensor_id,
            extruder_group,
            fields: fields.to_vec(),
        }
    }
}

impl fmt::Display for DeviceSensorStatusData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SensorStatus {
                sensor_id,
                extruder_group,
                fields,
            } => {
                if let Some(extruder_group) = extruder_group {
                    write!(
                        f,
                        "SensorStatus(sensor_id=0x{sensor_id:02x}, extruder_group={extruder_group}, fields={})",
                        hex_bytes(fields)
                    )
                } else {
                    write!(
                        f,
                        "SensorStatus(sensor_id=0x{sensor_id:02x}, fields={})",
                        hex_bytes(fields)
                    )
                }
            }
            Self::UnknownPayload(payload) => {
                write!(f, "UnknownPayload(payload={})", hex_bytes(payload))
            }
        }
    }
}
