//! tokenizer/mod.rs -- Unified tokenizer pipeline
//!
//! Combines dictionary analysis + codon substitution into
//! a single encode/decode interface for use by the frame layer.
//!
//! Encode pipeline:
//!   raw bytes -> build dictionary -> codon encode -> [dict header][tokenized]
//!
//! Decode pipeline:
//!   [dict header][tokenized] -> deserialize dict -> codon decode -> raw bytes

pub mod dictionary;
pub mod codon;

use dictionary::{build, serialize, deserialize, DictEntry};
use codon::{encode as codon_encode, decode as codon_decode};

/// Magic prefix for tokenized buffers.
/// Allows decoder to detect whether tokenization was applied.
const TOK_MAGIC: [u8; 4] = [0x47, 0x4E, 0x54, 0x4B]; // "GNTK"

/// Tokenizer statistics from last encode call.
#[derive(Debug, Default, Clone)]
pub struct TokenizerStats {
    pub dict_entries:    usize,
    pub input_bytes:     usize,
    pub tokenized_bytes: usize,
    pub estimated_saving: usize,
}

impl TokenizerStats {
    pub fn ratio(&self) -> f64 {
        if self.tokenized_bytes == 0 { return 1.0; }
        self.input_bytes as f64 / self.tokenized_bytes as f64
    }
}

/// Stateless tokenizer -- no per-instance state, all context
/// is carried in the encoded buffer itself (dictionary header).
pub struct Tokenizer;

impl Tokenizer {
    pub fn new() -> Self { Tokenizer }

    /// Encode: analyse buf, build dictionary, substitute codons.
    /// Returns (encoded_buffer, stats).
    pub fn encode(&self, buf: &[u8]) -> (Vec<u8>, TokenizerStats) {
        let entries = build(buf);
        let dict_header = serialize(&entries);
        let tokenized   = codon_encode(buf, &entries);

        let stats = TokenizerStats {
            dict_entries:     entries.len(),
            input_bytes:      buf.len(),
            tokenized_bytes:  tokenized.len(),
            estimated_saving: entries.iter().map(|e| e.saving).sum(),
        };

        // Frame: [4B magic][dict_header][4B original_len][tokenized]
        let mut out = Vec::with_capacity(
            4 + dict_header.len() + 4 + tokenized.len()
        );
        out.extend_from_slice(&TOK_MAGIC);
        out.extend_from_slice(&dict_header);
        out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
        out.extend_from_slice(&tokenized);

        (out, stats)
    }

    /// Decode: read dictionary header, reverse codon substitution.
    /// Returns decoded bytes or error.
    pub fn decode(&self, buf: &[u8]) -> Result<Vec<u8>, String> {
        if buf.len() < 4 {
            return Err("tokenizer: buffer too short".into());
        }

        // Check magic -- if absent, buffer was not tokenized, pass through
        if buf[0..4] != TOK_MAGIC {
            return Ok(buf.to_vec());
        }

        let (entries, dict_end) = deserialize(&buf[4..])
            .map_err(|e| format!("tokenizer: {e}"))?;

        let header_end = 4 + dict_end;
        if header_end + 4 > buf.len() {
            return Err("tokenizer: truncated original_len".into());
        }

        let original_len = u32::from_le_bytes(
            buf[header_end..header_end + 4].try_into().unwrap()
        ) as usize;

        let tokenized = &buf[header_end + 4..];
        let decoded   = codon_decode(tokenized, &entries);

        if decoded.len() != original_len {
            return Err(format!(
                "tokenizer: length mismatch: got {} expected {}",
                decoded.len(), original_len
            ));
        }
        Ok(decoded)
    }
}

impl Default for Tokenizer {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenizer() -> Tokenizer { Tokenizer::new() }

    #[test]
    fn test_roundtrip_repetitive() {
        let t   = tokenizer();
        let buf: Vec<u8> = "hello world ".repeat(50).into_bytes();
        let (enc, stats) = t.encode(&buf);
        let dec = t.decode(&enc).expect("decode failed");
        assert_eq!(dec, buf, "roundtrip failed");
        assert!(stats.dict_entries > 0);
        assert!(stats.ratio() > 1.0, "should compress: ratio={}", stats.ratio());
    }

    #[test]
    fn test_roundtrip_empty() {
        let t = tokenizer();
        let (enc, _) = t.encode(&[]);
        let dec = t.decode(&enc).expect("decode empty failed");
        assert_eq!(dec, b"");
    }

    #[test]
    fn test_roundtrip_no_repeats() {
        let t   = tokenizer();
        let buf: Vec<u8> = (0u8..=127).collect();
        let (enc, stats) = t.encode(&buf);
        let dec = t.decode(&enc).expect("decode failed");
        assert_eq!(dec, buf);
        // May not compress -- just verify lossless
        let _ = stats;
    }

    #[test]
    fn test_passthrough_unrecognized() {
        // Buffer without GNTK magic passes through decode unchanged
        let t   = tokenizer();
        let buf = b"raw unformatted data".to_vec();
        let dec = t.decode(&buf).expect("passthrough failed");
        assert_eq!(dec, buf);
    }

    #[test]
    fn test_stats_populated() {
        let t   = tokenizer();
        let buf: Vec<u8> = "repeated pattern ".repeat(30).into_bytes();
        let (_, stats) = t.encode(&buf);
        assert_eq!(stats.input_bytes, buf.len());
        assert!(stats.dict_entries > 0);
    }

    #[test]
    fn test_large_batch() {
        let t   = tokenizer();
        // Simulate 100 serialized messages ~800KB
        let msg = "user joined channel general timestamp 1743744000 payload data ";
        let buf: Vec<u8> = msg.repeat(500).into_bytes();
        let (enc, stats) = t.encode(&buf);
        let dec = t.decode(&enc).expect("large batch decode failed");
        assert_eq!(dec, buf, "large batch roundtrip failed");
        assert!(stats.ratio() > 1.0);
        println!("large batch: {}KB -> {}KB ratio={:.2}x dict={}", 
            buf.len()/1024, enc.len()/1024, stats.ratio(), stats.dict_entries);
    }
}
