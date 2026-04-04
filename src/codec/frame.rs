//! frame.rs -- GN frame format
//!
//! Wire layout:
//!   [0..4]       magic:   0x47 0x4E 0x4C 0x5A ("GNLZ")
//!   [4]          version: u8
//!   [5]          flags:   u8  (bit 0 = compression enabled)
//!   [6..10]      length:  u32 le (payload byte count)
//!   [10..10+len] payload: bytes
//!   [10+len..+4] crc32:   u32 le (over bytes 0..10+len)

use crate::codec::crc::crc32;

pub const MAGIC:   [u8; 4] = [0x47, 0x4E, 0x4C, 0x5A];
pub const VERSION: u8      = 2;
pub const FLAG_COMPRESSION: u8 = 0x01;

#[derive(Debug, PartialEq)]
pub struct Frame {
    pub version: u8,
    pub flags:   u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(payload: Vec<u8>, compressed: bool) -> Self {
        Frame {
            version: VERSION,
            flags: if compressed { FLAG_COMPRESSION } else { 0 },
            payload,
        }
    }
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSION != 0
    }
}

#[derive(Debug)]
pub enum FrameError {
    InvalidMagic,
    UnsupportedVersion(u8),
    Truncated,
    CrcMismatch { stored: u32, computed: u32 },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FrameError::InvalidMagic => write!(f, "invalid magic bytes"),
            FrameError::UnsupportedVersion(v) => write!(f, "unsupported version: {v}"),
            FrameError::Truncated => write!(f, "frame truncated"),
            FrameError::CrcMismatch { stored, computed } =>
                write!(f, "CRC32 mismatch: stored {stored:#010x} computed {computed:#010x}"),
        }
    }
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + frame.payload.len() + 4);
    out.extend_from_slice(&MAGIC);
    out.push(frame.version);
    out.push(frame.flags);
    let len = frame.payload.len() as u32;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&frame.payload);
    let crc = crc32(&out);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}

pub fn decode(data: &[u8]) -> Result<Frame, FrameError> {
    if data.len() < 14 { return Err(FrameError::Truncated); }
    if data[0..4] != MAGIC { return Err(FrameError::InvalidMagic); }
    let version = data[4];
    if version != VERSION { return Err(FrameError::UnsupportedVersion(version)); }
    let flags = data[5];
    let len = u32::from_le_bytes(data[6..10].try_into().map_err(|_| FrameError::Truncated)?) as usize;
    let payload_end = 10 + len;
    if data.len() < payload_end + 4 { return Err(FrameError::Truncated); }
    let stored   = u32::from_le_bytes(data[payload_end..payload_end+4].try_into().map_err(|_| FrameError::Truncated)?);
    let computed = crc32(&data[..payload_end]);
    if stored != computed { return Err(FrameError::CrcMismatch { stored, computed }); }
    Ok(Frame { version, flags, payload: data[10..payload_end].to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(payload: Vec<u8>, compressed: bool) {
        let frame   = Frame::new(payload.clone(), compressed);
        let encoded = encode(&frame);
        let decoded = decode(&encoded).expect("decode failed");
        assert_eq!(decoded.payload, payload);
        assert_eq!(decoded.is_compressed(), compressed);
    }

    #[test]
    fn test_roundtrip_uncompressed() { roundtrip(b"hello glasik".to_vec(), false); }

    #[test]
    fn test_roundtrip_compressed_flag() { roundtrip(b"compressed".to_vec(), true); }

    #[test]
    fn test_empty_payload() { roundtrip(vec![], false); }

    #[test]
    fn test_large_payload() {
        let payload: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
        roundtrip(payload, false);
    }

    #[test]
    fn test_magic_check() {
        let mut enc = encode(&Frame::new(b"data".to_vec(), false));
        enc[0] = 0x00;
        assert!(matches!(decode(&enc), Err(FrameError::InvalidMagic)));
    }

    #[test]
    fn test_crc_corruption_detected() {
        let mut enc = encode(&Frame::new(b"important".to_vec(), false));
        let last = enc.len();
        enc[last - 5] ^= 0xFF;
        assert!(matches!(decode(&enc), Err(FrameError::CrcMismatch { .. })));
    }

    #[test]
    fn test_truncated() {
        let enc = encode(&Frame::new(b"data".to_vec(), false));
        assert!(matches!(decode(&enc[..8]), Err(FrameError::Truncated)));
    }
}
