//! crc.rs -- CRC32 integrity verification
//!
//! Uses the standard IEEE polynomial (same as zlib, Ethernet).
//! Matches the JS reference implementation which uses SHA256[0..4]
//! reinterpreted as u32 -- we use real CRC32 here, which is faster
//! and more appropriate. Frame version distinguishes the two.

/// Compute CRC32 over a byte slice.
/// Uses lookup table for O(n) performance.
pub fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for i in 0..256u32 {
            let mut c = i;
            for _ in 0..8 {
                if c & 1 != 0 {
                    c = 0xEDB88320 ^ (c >> 1); // IEEE polynomial, reflected
                } else {
                    c >>= 1;
                }
            }
            t[i as usize] = c;
        }
        t
    });

    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = table[idx] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_vector() {
        // CRC32 of "123456789" = 0xCBF43926 (standard test vector)
        let result = crc32(b"123456789");
        assert_eq!(result, 0xCBF43926, "CRC32 standard vector failed");
    }

    #[test]
    fn test_empty() {
        // CRC32 of empty = 0x00000000
        assert_eq!(crc32(b""), 0x00000000);
    }

    #[test]
    fn test_deterministic() {
        let a = crc32(b"glasik");
        let b = crc32(b"glasik");
        assert_eq!(a, b);
    }

    #[test]
    fn test_different_inputs() {
        assert_ne!(crc32(b"glasik"), crc32(b"Glasik"));
    }

    #[test]
    fn test_integrity_detection() {
        let data = b"GN frame payload";
        let good = crc32(data);
        let mut corrupted = data.to_vec();
        corrupted[4] ^= 0xFF; // flip bits in one byte
        let bad = crc32(&corrupted);
        assert_ne!(good, bad, "corruption not detected");
    }
}
