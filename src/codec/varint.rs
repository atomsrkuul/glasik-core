//! varint.rs -- Variable-length integer encoding
//!
//! Encodes unsigned integers using 7 bits per byte.
//! The high bit (0x80) signals "more bytes follow".
//!
//! Example:
//!   300 = 0b100101100
//!       → [0b10101100, 0b00000010]
//!         [0xAC,       0x02      ]
//!          ^^^^ lower 7 bits     ^^^^ upper bits, no continuation

/// Encode a u64 into varint bytes, appended to `out`.
pub fn encode(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);         // final byte — high bit clear
            break;
        } else {
            out.push(byte | 0x80); // more bytes follow — set high bit
        }
    }
}

/// Decode a varint from `data` starting at `pos`.
/// Returns (value, new_pos) or an error string.
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
            return Ok((value, cur)); // high bit clear = final byte
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(n: u64) {
        let mut buf = Vec::new();
        encode(n, &mut buf);
        let (decoded, pos) = decode(&buf, 0).expect("decode failed");
        assert_eq!(decoded, n, "roundtrip failed for {n}");
        assert_eq!(pos, buf.len(), "pos mismatch for {n}");
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
        // Multiple varints in a single buffer
        let mut buf = Vec::new();
        encode(1,   &mut buf);
        encode(300, &mut buf);
        encode(0,   &mut buf);

        let (a, p1) = decode(&buf, 0).unwrap();
        let (b, p2) = decode(&buf, p1).unwrap();
        let (c, _)  = decode(&buf, p2).unwrap();

        assert_eq!(a, 1);
        assert_eq!(b, 300);
        assert_eq!(c, 0);
    }

    #[test]
    fn test_known_encoding() {
        // 300 = [0xAC, 0x02] — verify against JS reference
        let mut buf = Vec::new();
        encode(300, &mut buf);
        assert_eq!(buf, vec![0xAC, 0x02]);
    }
}
