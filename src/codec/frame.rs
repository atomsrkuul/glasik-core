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

pub const MAGIC: [u8; 4] = [0x47, 0x4E, 0x4C, 0x5A];
pub const VERSION: u8 = 2;
pub const FLAG_COMPRESSION: u8 = 0x01;
pub const HEADER_LEN: usize = 10; // magic+ver+flags+len
pub const TRAILER_LEN: usize = 4; // crc32
pub const MIN_FRAME_LEN: usize = HEADER_LEN + TRAILER_LEN;

/// Owned frame -- used when building frames for transmission.
#[derive(Debug, PartialEq)]
pub struct Frame {
    pub version: u8,
    pub flags: u8,
    pub payload: Vec<u8>,
}

/// Zero-copy decoded frame -- borrows directly from source buffer.
/// Lifetime tied to the buffer it was decoded from.
/// No heap allocation on decode path.
#[derive(Debug, PartialEq)]
pub struct FrameView<'a> {
    pub version: u8,
    pub flags: u8,
    pub payload: &'a [u8], // slice into original buffer
}

impl<'a> FrameView<'a> {
    #[inline]
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSION != 0
    }
    /// Promote to owned Frame when you need to store or mutate.
    pub fn to_owned(&self) -> Frame {
        Frame {
            version: self.version,
            flags: self.flags,
            payload: self.payload.to_vec(),
        }
    }
}

impl Frame {
    pub fn new(payload: Vec<u8>, compressed: bool) -> Self {
        Frame {
            version: VERSION,
            flags: if compressed { FLAG_COMPRESSION } else { 0 },
            payload,
        }
    }
    #[inline]
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
            FrameError::CrcMismatch { stored, computed } => write!(
                f,
                "CRC32 mismatch: stored {stored:#010x} computed {computed:#010x}"
            ),
        }
    }
}

/// Encode an owned Frame into bytes. Allocates output buffer.
pub fn encode(frame: &Frame) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + frame.payload.len() + TRAILER_LEN);
    out.extend_from_slice(&MAGIC);
    out.push(frame.version);
    out.push(frame.flags);
    out.extend_from_slice(&(frame.payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&frame.payload);
    let crc = crc32(&out);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}

/// Decode bytes into a FrameView. Zero allocation -- payload is a
/// slice into `data`. Verifies magic, version, and CRC32.
pub fn decode_view(data: &[u8]) -> Result<FrameView<'_>, FrameError> {
    if data.len() < MIN_FRAME_LEN {
        return Err(FrameError::Truncated);
    }
    if data[0..4] != MAGIC {
        return Err(FrameError::InvalidMagic);
    }
    let version = data[4];
    if version != VERSION {
        return Err(FrameError::UnsupportedVersion(version));
    }
    let flags = data[5];
    let len =
        u32::from_le_bytes(data[6..10].try_into().map_err(|_| FrameError::Truncated)?) as usize;
    let payload_end = HEADER_LEN + len;
    if data.len() < payload_end + TRAILER_LEN {
        return Err(FrameError::Truncated);
    }
    let stored = u32::from_le_bytes(
        data[payload_end..payload_end + TRAILER_LEN]
            .try_into()
            .map_err(|_| FrameError::Truncated)?,
    );
    let computed = crc32(&data[..payload_end]);
    if stored != computed {
        return Err(FrameError::CrcMismatch { stored, computed });
    }
    Ok(FrameView {
        version,
        flags,
        payload: &data[HEADER_LEN..payload_end], // zero-copy slice
    })
}

/// Decode into an owned Frame. Allocates. Use when you need to store
/// the payload beyond the lifetime of the source buffer.
#[inline]
pub fn decode(data: &[u8]) -> Result<Frame, FrameError> {
    decode_view(data).map(|v| v.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(payload: Vec<u8>, compressed: bool) {
        let frame = Frame::new(payload.clone(), compressed);
        let encoded = encode(&frame);

        // Zero-copy path
        let view = decode_view(&encoded).expect("decode_view failed");
        assert_eq!(view.payload, payload.as_slice());
        assert_eq!(view.is_compressed(), compressed);

        // Owned path
        let owned = decode(&encoded).expect("decode failed");
        assert_eq!(owned.payload, payload);
    }

    #[test]
    fn test_roundtrip_uncompressed() {
        roundtrip(b"hello glasik".to_vec(), false);
    }

    #[test]
    fn test_roundtrip_compressed_flag() {
        roundtrip(b"compressed".to_vec(), true);
    }

    #[test]
    fn test_empty_payload() {
        roundtrip(vec![], false);
    }

    #[test]
    fn test_large_payload() {
        let payload: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
        roundtrip(payload, false);
    }

    #[test]
    fn test_magic_check() {
        let mut enc = encode(&Frame::new(b"data".to_vec(), false));
        enc[0] = 0x00;
        assert!(matches!(decode_view(&enc), Err(FrameError::InvalidMagic)));
    }

    #[test]
    fn test_crc_corruption_detected() {
        let mut enc = encode(&Frame::new(b"important".to_vec(), false));
        let last = enc.len();
        enc[last - 5] ^= 0xFF;
        assert!(matches!(
            decode_view(&enc),
            Err(FrameError::CrcMismatch { .. })
        ));
    }

    #[test]
    fn test_truncated() {
        let enc = encode(&Frame::new(b"data".to_vec(), false));
        assert!(matches!(decode_view(&enc[..8]), Err(FrameError::Truncated)));
    }

    #[test]
    fn test_zero_copy_no_alloc() {
        // FrameView payload is a slice into encoded -- same pointer
        let payload = b"zero copy test".to_vec();
        let encoded = encode(&Frame::new(payload.clone(), false));
        let view = decode_view(&encoded).unwrap();
        // Verify the slice points into encoded, not a copy
        let view_ptr = view.payload.as_ptr();
        let encoded_ptr = encoded[HEADER_LEN..].as_ptr();
        assert_eq!(
            view_ptr, encoded_ptr,
            "payload should be a slice, not a copy"
        );
    }

    #[test]
    fn test_promote_to_owned() {
        let payload = b"promote me".to_vec();
        let encoded = encode(&Frame::new(payload.clone(), true));
        let view = decode_view(&encoded).unwrap();
        let owned = view.to_owned();
        assert_eq!(owned.payload, payload);
        assert!(owned.is_compressed());
    }
}
