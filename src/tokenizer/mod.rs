//! tokenizer/mod.rs -- Unified tokenizer pipeline
//!
//! Two-pass encoding:
//!   Pass 1: build dict from full buffer, substitute tokens
//!   Pass 2: build dict from residual (non-token bytes) only,
//!            apply to residual regions, merge dicts
//!
//! Decode: single pass using merged dictionary.
//! Clean boundary -- second pass never touches token regions.

pub mod codon;
pub mod dictionary;
pub mod preseed;
pub mod sliding;
pub mod sliding_v2;

use codon::{decode as codon_decode, encode as codon_encode, ESCAPE};
use dictionary::{build, build_second_pass, deserialize, extract_residual, serialize};

pub const TOK_MAGIC: [u8; 4] = [0x47, 0x4E, 0x54, 0x4B];

#[derive(Debug, Default, Clone)]
pub struct TokenizerStats {
    pub dict_entries: usize,
    pub pass1_entries: usize,
    pub pass2_entries: usize,
    pub input_bytes: usize,
    pub tokenized_bytes: usize,
    pub estimated_saving: usize,
}

impl TokenizerStats {
    pub fn ratio(&self) -> f64 {
        if self.tokenized_bytes == 0 {
            return 1.0;
        }
        self.input_bytes as f64 / self.tokenized_bytes as f64
    }
}

/// Remove dictionary entries that produced no tokens in the encoded output.
/// Reduces header overhead by only serializing entries that actually fired.
fn prune_to_fired(entries: &[dictionary::DictEntry], encoded: &[u8]) -> Vec<dictionary::DictEntry> {
    use codon::ESCAPE;
    // Count which token IDs appear in the encoded output
    let mut fired = vec![false; entries.len()];
    let mut i = 0;
    while i < encoded.len() {
        if encoded[i] == ESCAPE && i + 1 < encoded.len() {
            let id = encoded[i + 1] as usize;
            if id > 0 && id - 1 < fired.len() {
                fired[id - 1] = true;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    entries
        .iter()
        .enumerate()
        .filter(|(i, _)| fired[*i])
        .map(|(_, e)| e.clone())
        .collect()
}

pub struct Tokenizer;

impl Tokenizer {
    pub fn new() -> Self {
        Tokenizer
    }

    /// Two-pass encode with category-aware pre-seeding.
    /// Returns (encoded_buffer, stats).
    pub fn encode(&self, buf: &[u8]) -> (Vec<u8>, TokenizerStats) {
        // ── Category detection + preseed ─────────────────────────────────
        let category = crate::tokenizer::preseed::detect(buf);
        let mut pass1 = crate::tokenizer::preseed::preseed(&category);

        // ── Pass 1: merge preseed with adaptive dict ─────────────────────
        let adaptive = build(buf);
        let preseed_bytes: std::collections::HashSet<Vec<u8>> =
            pass1.iter().map(|e| e.bytes.clone()).collect();
        for entry in adaptive {
            if preseed_bytes.contains(&entry.bytes) {
                continue;
            }
            pass1.push(entry);
        }
        pass1.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
        pass1.truncate(dictionary::MAX_ENTRIES);

        // Dry-run to find which entries fire, then prune and re-encode.
        // This removes unfired entries from the header without breaking IDs.
        let tokenized1 = codon_encode(buf, &pass1);

        // ── Pass 2: residual only ────────────────────────────────────────
        let residual = extract_residual(&tokenized1);
        let pass2 = build_second_pass(&residual, pass1.len());

        // ── Apply pass 2 to non-token regions of tokenized1 ─────────────
        let tokenized2 = if pass2.is_empty() {
            tokenized1.clone()
        } else {
            apply_pass2(&tokenized1, &pass2, pass1.len())
        };

        // ── Merge dicts for header ───────────────────────────────────────
        let mut merged = pass1.clone();
        merged.extend(pass2.iter().cloned());
        let dict_header = serialize(&merged);

        let stats = TokenizerStats {
            dict_entries: merged.len(),
            pass1_entries: pass1.len(),
            pass2_entries: pass2.len(),
            input_bytes: buf.len(),
            tokenized_bytes: tokenized2.len(),
            estimated_saving: merged.iter().map(|e| e.saving).sum(),
        };

        let mut out = Vec::with_capacity(4 + dict_header.len() + 4 + tokenized2.len());
        out.extend_from_slice(&TOK_MAGIC);
        out.extend_from_slice(&dict_header);
        out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
        out.extend_from_slice(&tokenized2);

        (out, stats)
    }

    pub fn decode(&self, buf: &[u8]) -> Result<Vec<u8>, String> {
        if buf.len() < 4 {
            return Err("tokenizer: buffer too short".into());
        }
        if buf[0..4] != TOK_MAGIC {
            return Ok(buf.to_vec());
        }

        let (entries, dict_end) = deserialize(&buf[4..]).map_err(|e| format!("tokenizer: {e}"))?;
        let header_end = 4 + dict_end;
        if header_end + 4 > buf.len() {
            return Err("tokenizer: truncated".into());
        }
        let original_len =
            u32::from_le_bytes(buf[header_end..header_end + 4].try_into().unwrap()) as usize;
        let tokenized = &buf[header_end + 4..];
        let decoded = codon_decode(tokenized, &entries);
        if decoded.len() != original_len {
            return Err(format!(
                "tokenizer: length mismatch got {} expected {}",
                decoded.len(),
                original_len
            ));
        }
        Ok(decoded)
    }
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply pass2 dictionary only to non-token regions of a tokenized buffer.
/// Token regions (ESCAPE sequences) are copied unchanged.
/// Pass2 token IDs are offset by pass1_count to avoid collision.
fn apply_pass2(buf: &[u8], pass2: &[dictionary::DictEntry], pass1_count: usize) -> Vec<u8> {
    // Extract non-token spans with their positions
    // Then apply codon_encode to each span with offset IDs
    let mut out = Vec::with_capacity(buf.len());
    let mut i = 0usize;

    // Build pass2 index with ID offset
    let offset_entries: Vec<dictionary::DictEntry> = pass2.iter().cloned().collect();

    while i < buf.len() {
        if buf[i] == ESCAPE && i + 1 < buf.len() {
            // Pass through existing token unchanged
            out.push(buf[i]);
            out.push(buf[i + 1]);
            i += 2;
        } else {
            // Collect non-token span
            let start = i;
            while i < buf.len() && !(buf[i] == ESCAPE && i + 1 < buf.len()) {
                i += 1;
            }
            let span = &buf[start..i];
            // Encode span with pass2 dict, then offset token IDs
            let encoded = codon_encode(span, &offset_entries);
            // Offset: pass2 token id N -> actual id N + pass1_count
            let mut j = 0;
            while j < encoded.len() {
                if encoded[j] == ESCAPE && j + 1 < encoded.len() {
                    let id = encoded[j + 1];
                    if id == 0x00 {
                        out.push(ESCAPE);
                        out.push(0x00);
                    } else {
                        // Offset the id
                        out.push(ESCAPE);
                        let offset_id = id as u16 + pass1_count as u16;
                        debug_assert!(offset_id <= 255, "token ID overflow: {id} + {pass1_count}");
                        out.push(offset_id as u8);
                    }
                    j += 2;
                } else {
                    out.push(encoded[j]);
                    j += 1;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok() -> Tokenizer {
        Tokenizer::new()
    }

    #[test]
    fn test_roundtrip_repetitive() {
        let t = tok();
        let buf: Vec<u8> = "hello world ".repeat(50).into_bytes();
        let (enc, stats) = t.encode(&buf);
        let dec = t.decode(&enc).expect("decode failed");
        assert_eq!(dec, buf);
        println!(
            "pass1={} pass2={} ratio={:.3}x",
            stats.pass1_entries,
            stats.pass2_entries,
            stats.ratio()
        );
        assert!(stats.ratio() > 1.0);
    }

    #[test]
    fn test_roundtrip_empty() {
        let t = tok();
        let (enc, _) = t.encode(&[]);
        assert_eq!(t.decode(&enc).unwrap(), b"");
    }

    #[test]
    fn test_roundtrip_no_repeats() {
        let t = tok();
        let buf: Vec<u8> = (0u8..=127).collect();
        let (enc, _) = t.encode(&buf);
        assert_eq!(t.decode(&enc).unwrap(), buf);
    }

    #[test]
    fn test_two_pass_lossless() {
        let t = tok();
        let buf: Vec<u8> = "alpha beta gamma delta epsilon ".repeat(40).into_bytes();
        let (enc, stats) = t.encode(&buf);
        let dec = t.decode(&enc).expect("two-pass decode failed");
        assert_eq!(dec, buf, "two-pass roundtrip failed");
        println!(
            "two-pass: p1={} p2={} ratio={:.3}x input={}B tok={}B",
            stats.pass1_entries,
            stats.pass2_entries,
            stats.ratio(),
            stats.input_bytes,
            stats.tokenized_bytes
        );
    }

    #[test]
    fn test_passthrough_unrecognized() {
        let t = tok();
        let buf = b"raw unformatted data".to_vec();
        assert_eq!(t.decode(&buf).unwrap(), buf);
    }

    #[test]
    fn test_large_batch() {
        let t = tok();
        let msg = "user joined channel general timestamp 1743744000 payload data ";
        let buf: Vec<u8> = msg.repeat(500).into_bytes();
        let (enc, stats) = t.encode(&buf);
        let dec = t.decode(&enc).expect("large batch failed");
        assert_eq!(dec, buf);
        assert!(stats.ratio() > 1.0);
        println!(
            "large: {}KB->{}KB ratio={:.2}x p1={} p2={}",
            buf.len() / 1024,
            enc.len() / 1024,
            stats.ratio(),
            stats.pass1_entries,
            stats.pass2_entries
        );
    }
}

pub mod lz77_gn;
