use bytes::{Buf, BufMut, BytesMut};
use std::io;
use thiserror::Error;

use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;

pub use crate::consts::*;

#[derive(Debug, PartialEq)]
pub enum SDMessage {
    Error {
        msg: String,
    },
    Plate {
        plate: String,
        timestamp: u32,
    },
    Ticket {
        plate: String,
        road: u16,
        mile1: u16,
        timestamp1: u32,
        mile2: u16,
        timestamp2: u32,
        speed: u16,
    },
    WantHeartbeat {
        interval: u32,
    },
    Heartbeat,
    IAmCamera {
        road: u16,
        mile: u16,
        limit: u16,
    },
    IAmDispatcher {
        roads: Vec<u16>,
    },
}

#[derive(Debug)]
pub struct SpeedDaemonCodec {}

#[derive(Debug, Error)]
pub enum SpeedDaemonCodecError {
    #[error("IO error")]
    IoError(io::Error),
}

/*
impl fmt::Display for SpeedDaemonCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeedDaemonCodecError::IoError(e) => write!(f, "{}", e),
        }
    }
}
*/

impl From<io::Error> for SpeedDaemonCodecError {
    fn from(e: io::Error) -> SpeedDaemonCodecError {
        SpeedDaemonCodecError::IoError(e)
    }
}
/*
impl std::error::Error for SpeedDaemonCodecError {}
*/
impl SpeedDaemonCodec {
    pub fn new() -> SpeedDaemonCodec {
        SpeedDaemonCodec {}
    }
}

impl Default for SpeedDaemonCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder<SDMessage> for SpeedDaemonCodec {
    type Error = SpeedDaemonCodecError;

    fn encode(&mut self, item: SDMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            SDMessage::Error { msg } => {
                if msg.len() > 255 {
                    return Err(SpeedDaemonCodecError::IoError(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Frame of length {} is too large.", msg.len()),
                    )));
                }

                dst.reserve(2 + msg.len());
                dst.put_u8(SD_ERROR);
                dst.put_u8(msg.len() as u8);
                dst.put(msg.as_bytes());
            }
            SDMessage::Heartbeat => {
                dst.reserve(1);
                dst.put_u8(SD_HEARTBEAT);
            }
            SDMessage::Ticket {
                plate,
                road,
                mile1,
                timestamp1,
                mile2,
                timestamp2,
                speed,
            } => {
                dst.reserve(1 + 1 + plate.len() + 2 + 2 + 4 + 2 + 4 + 2);
                dst.put_u8(SD_TICKET);

                dst.put_u8(plate.len() as u8);
                dst.put(plate.as_bytes());

                dst.put_u16(road);
                dst.put_u16(mile1);
                dst.put_u32(timestamp1);
                dst.put_u16(mile2);
                dst.put_u32(timestamp2);
                dst.put_u16(speed);
            }
            SDMessage::IAmCamera { road, mile, limit } => {
                dst.reserve(1 + 2 + 2 + 2);
                dst.put_u8(SD_IAMCAMERA);

                dst.put_u16(road);
                dst.put_u16(mile);
                dst.put_u16(limit);
            }
            SDMessage::IAmDispatcher { roads } => {
                dst.reserve(1 + 1 + (roads.len() * 2));
                dst.put_u8(SD_IAMDISPATCHER);

                dst.put_u8(roads.len() as u8);
                for road in roads {
                    dst.put_u16(road);
                }
            }
            SDMessage::Plate { plate, timestamp } => {
                dst.reserve(1 + 1 + plate.len() + 4);
                dst.put_u8(SD_PLATE);

                dst.put_u8(plate.len() as u8);
                dst.put(plate.as_bytes());

                dst.put_u32(timestamp);
            }
            _ => {
                return Err(SpeedDaemonCodecError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unknown command: {:?} .", item),
                )));
            }
        }
        Ok(())
    }
}

impl Decoder for SpeedDaemonCodec {
    type Error = SpeedDaemonCodecError;
    type Item = SDMessage;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            /* Not enough data to read message type */
            return Ok(None);
        }

        let msg_type = src.first();
        match msg_type {
            Some(&SD_WANTHEARTBEAT) => {
                if src.len() < (1 + 4) {
                    /* Not enough data */
                    return Ok(None);
                }

                let _msg_type = src.get_u8();
                Ok(Some(SDMessage::WantHeartbeat {
                    interval: src.get_u32(),
                }))
            }
            Some(&SD_PLATE) => {
                if src.len() < (1 + 1) {
                    /* Not enough data */
                    return Ok(None);
                }

                let plate_len = *src.get(1).unwrap();
                if src.len() < ((1 + 1 + plate_len + 4).into()) {
                    /* Not enough data */
                    return Ok(None);
                }

                let _msg_type = src.get_u8();
                let _plate_len = src.get_u8();

                let plate = src.get(0..(plate_len).into()).unwrap();
                let plate = String::from_utf8_lossy(plate).to_string();
                src.advance(plate_len.into());

                Ok(Some(SDMessage::Plate {
                    plate,
                    timestamp: src.get_u32(),
                }))
            }
            Some(&SD_IAMCAMERA) => {
                if src.len() < (1 + 2 + 2 + 2) {
                    /* Not enough data */
                    return Ok(None);
                }

                let _msg_type = src.get_u8();
                Ok(Some(SDMessage::IAmCamera {
                    road: src.get_u16(),
                    mile: src.get_u16(),
                    limit: src.get_u16(),
                }))
            }
            Some(&SD_IAMDISPATCHER) => {
                if src.len() < (1 + 1) {
                    /* Not enough data */
                    return Ok(None);
                }

                let numroads = *src.get(1).unwrap();
                if src.len() < (1 + 1 + (numroads * 2)).into() {
                    /* Not enough data */
                    return Ok(None);
                }

                /* TODO: refactor to more idiomatic Rust */
                let _msg_type = src.get_u8();
                let _num_roads = src.get_u8();

                let mut roads: Vec<u16> = Vec::new();
                for _ in 0..numroads {
                    let road = src.get_u16();
                    roads.push(road);
                }

                Ok(Some(SDMessage::IAmDispatcher { roads }))
            }
            Some(&SD_ERROR) => Err(SpeedDaemonCodecError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Should not receive Error msg: {:?}", msg_type),
            ))),
            Some(&SD_TICKET) => {
                if src.len() < (1 + 1) {
                    /* Not enough data */
                    return Ok(None);
                }

                let plate_len = *src.get(1).unwrap();
                if src.len() < ((1 + 1 + plate_len + 4).into()) {
                    /* Not enough data */
                    return Ok(None);
                }

                let _msg_type = src.get_u8();
                let _plate_len = src.get_u8();

                let plate = src.get(0..(plate_len).into()).unwrap();
                let plate = String::from_utf8_lossy(plate).to_string();
                src.advance(plate_len.into());

                let road = src.get_u16();
                let mile1 = src.get_u16();
                let timestamp1 = src.get_u32();
                let mile2 = src.get_u16();
                let timestamp2 = src.get_u32();
                let speed = src.get_u16();

                Ok(Some(SDMessage::Ticket {
                    plate,
                    road,
                    mile1,
                    timestamp1,
                    mile2,
                    timestamp2,
                    speed,
                }))
            }
            Some(&SD_HEARTBEAT) => Err(SpeedDaemonCodecError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Should not receive Heartbeat msg: {:?}", msg_type),
            ))),
            _ => Err(SpeedDaemonCodecError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown command: {:?}", msg_type),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_err() {
        let data = SDMessage::Error {
            msg: "illegal msg".to_string(),
        };

        let mut codec = SpeedDaemonCodec::new();
        let mut output_buf = BytesMut::with_capacity(128);

        match codec.encode(data, &mut output_buf) {
            Ok(()) => {}
            _ => panic!("Non OK value returned from encode"),
        }

        let encoded_msg = output_buf.freeze();
        assert_eq!(
            encoded_msg.to_vec(),
            b"\x10\x0b\x69\x6c\x6c\x65\x67\x61\x6c\x20\x6d\x73\x67"
        );
    }

    #[test]
    fn test_msg_plate() {
        let data = b"\x20\x04\x55\x4e\x31\x58\x00\x00\x03\xe8";

        let mut codec = SpeedDaemonCodec::new();
        let mut input_buf = BytesMut::with_capacity(128);
        input_buf.put_slice(data);

        let output = codec.decode(&mut input_buf);
        match output {
            Err(e) => panic!("Decoding message failed: {:?}", e),
            Ok(None) => panic!("Decoding message failed: not enough data?"),
            _ => {}
        }

        if let Ok(Some(output)) = output {
            assert_eq!(
                output,
                SDMessage::Plate {
                    plate: "UN1X".to_string(),
                    timestamp: 1000,
                }
            );
        }
    }

    #[test]
    fn test_msg_ticket() {
        let data = SDMessage::Ticket {
            plate: "RE05BKG".to_string(),
            road: 368,
            mile1: 1234,
            timestamp1: 1000000,
            mile2: 1235,
            timestamp2: 1000060,
            speed: 6000,
        };

        let mut codec = SpeedDaemonCodec::new();
        let mut output_buf = BytesMut::with_capacity(128);

        match codec.encode(data, &mut output_buf) {
            Ok(()) => {}
            _ => panic!("Non OK value returned from encode"),
        }

        let encoded_msg = output_buf.freeze();
        assert_eq!(encoded_msg.to_vec(), b"\x21\x07\x52\x45\x30\x35\x42\x4b\x47\x01\x70\x04\xd2\x00\x0f\x42\x40\x04\xd3\x00\x0f\x42\x7c\x17\x70");
    }

    #[test]
    fn test_msg_wantheartbeat() {
        let data = b"\x40\x00\x00\x04\xdb";

        let mut codec = SpeedDaemonCodec::new();
        let mut input_buf = BytesMut::with_capacity(128);
        input_buf.put_slice(data);

        let output = codec.decode(&mut input_buf);
        match output {
            Err(e) => panic!("Decoding message failed: {:?}", e),
            Ok(None) => panic!("Decoding message failed: not enough data?"),
            _ => {}
        }

        if let Ok(Some(output)) = output {
            assert_eq!(output, SDMessage::WantHeartbeat { interval: 1243 });
        }
    }

    #[test]
    fn test_msg_heartbeat() {
        let data = SDMessage::Heartbeat;

        let mut codec = SpeedDaemonCodec::new();
        let mut output_buf = BytesMut::with_capacity(128);

        match codec.encode(data, &mut output_buf) {
            Ok(()) => {}
            _ => panic!("Non OK value returned from encode"),
        }

        let encoded_msg = output_buf.freeze();
        assert_eq!(encoded_msg.to_vec(), b"\x41");
    }

    #[test]
    fn test_msg_iamcamera() {
        let data = b"\x80\x01\x70\x04\xd2\x00\x28";

        let mut codec = SpeedDaemonCodec::new();
        let mut input_buf = BytesMut::with_capacity(128);
        input_buf.put_slice(data);

        let output = codec.decode(&mut input_buf);
        match output {
            Err(e) => panic!("Decoding message failed: {:?}", e),
            Ok(None) => panic!("Decoding message failed: not enough data?"),
            _ => {}
        }

        if let Ok(Some(output)) = output {
            assert_eq!(
                output,
                SDMessage::IAmCamera {
                    road: 368,
                    mile: 1234,
                    limit: 40,
                }
            );
        }
    }

    #[test]
    fn test_msg_iamdispatcher() {
        let data = b"\x81\x03\x00\x42\x01\x70\x13\x88";

        let mut codec = SpeedDaemonCodec::new();
        let mut input_buf = BytesMut::with_capacity(128);
        input_buf.put_slice(data);

        let output = codec.decode(&mut input_buf);
        match output {
            Err(e) => panic!("Decoding message failed: {:?}", e),
            Ok(None) => panic!("Decoding message failed: not enough data?"),
            _ => {}
        }

        if let Ok(Some(output)) = output {
            assert_eq!(
                output,
                SDMessage::IAmDispatcher {
                    roads: vec!(66, 368, 5000),
                }
            );
        }
    }
}
