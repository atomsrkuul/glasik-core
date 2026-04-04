//! varint.rs -- Variable-length integer encoding
//!
//! Encodes unsigned integers using 7 bits per byte.
//! The high bit (0x80) signals "more bytes follow".
//!
//! Example:
//!   300 = 0b100101100
//!       -> [0xAC, 0x02]
//!           lower 7 bits, upper bits no continuation

/// Maximum bytes a u64 varint can occupy (ceil(64/7) = 10)
pub const MAX_VARINT_LEN: usize = 10;

/// Stack-allocated varint output. Zero heap allocation.
/// Use .as_slice() to get the encoded bytes.
pub struct VarintBuf {
    data: [u8; MAX_VARINT_LEN],
    len:  usize,
}

impl VarintBuf {
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }
    #[inline]
    pub fn len(&self) -> usize { self.len }
}

/// Encode a u64 into a stack-allocated VarintBuf. Zero allocation.
/// Hot path -- called once per message field.
#[inline]
pub fn encode_stack(mut value: u64) -> VarintBuf {
    let mut buf = VarintBuf { data: [0u8; MAX_VARINT_LEN], len: 0 };
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.data[buf.len] = byte;
            buf.len += 1;
            break;
        } else {
            buf.data[buf.len] = byte | 0x80;
            buf.len += 1;
        }
    }
    buf
}

/// Encode a u64 varint, appending to an existing Vec<u8>.
/// Convenience wrapper around encode_stack for batch encoding.
#[inline]
pub fn encode(value: u64, out: &mut Vec<u8>) {
    let buf = encode_stack(value);
    out.extend_from_slice(buf.as_slice());
}

/// Decode a varint from `data` starting at `pos`.
/// Returns (value, new_pos) or an error string.
#[inline]
pub fn decode(data: &[u8], pos: usize) -> Result<(u64, usize), String> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut cur   = pos;
    loop {
        if cur >= data.len() {
            return Err(format!("varint: unexpected end of data at pos {cur}"));
        }
        if shift >= 64 {
            return Err("varint: overflow (>64 bits)".into());
        }
        let byte = data[cur] as u64;
        cur += 1;
        value |= (byte & 0x7F) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Ok((value, cur));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(n: u64) {
        // Test both paths
        let stack = encode_stack(n);
        let (decoded, pos) = decode(stack.as_slice(), 0).expect("decode failed");
        assert_eq!(decoded, n, "roundtrip failed for {n}");
        assert_eq!(pos, stack.len(), "pos mismatch for {n}");

        // Vec path produces identical bytes
        let mut vec_out = Vec::new();
        encode(n, &mut vec_out);
        assert_eq!(vec_out, stack.as_slice(), "stack/vec mismatch for {n}");
    }

    #[test]
    fn test_zero()        { roundtrip(0); }

    #[test]
    fn test_small()       { roundtrip(1); roundtrip(127); }

    #[test]
    fn test_two_bytes()   { roundtrip(128); roundtrip(300); roundtrip(16383); }

    #[test]
    fn test_large()       { roundtrip(u32::MAX as u64); roundtrip(u64::MAX); }

    #[test]
    fn test_sequential() {
        let mut buf = Vec::new();
        encode(1,   &mut buf);
        encode(300, &mut buf);
        encode(0,   &mut buf);
        let (a, p1) = decode(&buf, 0).unwrap();
        let (b, p2) = decode(&buf, p1).unwrap();
        let (c, _)  = decode(&buf, p2).unwrap();
        assert_eq!((a, b, c), (1, 300, 0));
    }

    #[test]
    fn test_known_encoding() {
        let buf = encode_stack(300);
        assert_eq!(buf.as_slice(), &[0xAC, 0x02]);
    }

    #[test]
    fn test_stack_no_alloc() {
        // encode_stack must fit in MAX_VARINT_LEN bytes for u64::MAX
        let buf = encode_stack(u64::MAX);
        assert!(buf.len() <= MAX_VARINT_LEN);
        let (v, _) = decode(buf.as_slice(), 0).unwrap();
        assert_eq!(v, u64::MAX);
    }
}
