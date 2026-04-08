//! codon.rs -- Codon-table symbol substitution
//!
//! A codon is a fixed-width 2-byte symbol that maps to a variable-length
//! byte sequence. Inspired by biological codon tables where 3-base codons
//! map to amino acids -- fixed-width symbols encoding variable semantics.
//!
//! Encoding:
//!   ESCAPE byte (0x01) followed by id byte (1..=N) -> dictionary entry N-1
//!   ESCAPE byte followed by 0x00                   -> literal 0x01 byte
//!
//! Substitution uses a first-byte index for O(1) bucket lookup,
//! then indexOf-style scanning to find match positions in bulk.
//! This avoids byte-by-byte iteration over the full buffer.

use super::dictionary::DictEntry;

pub const ESCAPE: u8 = 0x01;

/// First-byte index: maps first byte of each pattern to candidate list.
/// First-byte index: maps first byte of each pattern to candidate list.
#[derive(Debug)]
pub struct FirstByteIndex {
    buckets: Vec<Vec<(Vec<u8>, u8)>>,
}
impl FirstByteIndex {
    pub fn build(entries: &[DictEntry]) -> Self {
        let mut buckets: Vec<Vec<(Vec<u8>, u8)>> = vec![Vec::new(); 256];
        for (id, entry) in entries.iter().enumerate() {
            if entry.bytes.is_empty() { continue; }
            let first = entry.bytes[0] as usize;
            buckets[first].push((entry.bytes.clone(), (id + 1) as u8));
        }
        for bucket in &mut buckets {
            bucket.sort_unstable_by(|a, b| b.0.len().cmp(&a.0.len()));
        }
        FirstByteIndex { buckets }
    }
    pub fn find_matches(&self, buf: &[u8]) -> Vec<(usize, u8, usize)> {
        let mut matches: Vec<(usize, u8, usize)> = Vec::new();
        for bucket in &self.buckets {
            for (pattern, token_id) in bucket {
                if pattern.is_empty() || pattern.len() > buf.len() { continue; }
                let mut start = 0;
                while start + pattern.len() <= buf.len() {
                    if let Some(rel) = buf[start..].windows(pattern.len())
                        .position(|w| w == pattern.as_slice()) {
                        let pos = start + rel;
                        if buf[pos] != ESCAPE {
                            matches.push((pos, *token_id, pattern.len()));
                        }
                        start = pos + pattern.len();
                    } else { break; }
                }
            }
        }
        matches.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.2.cmp(&a.2)));
        matches
    }
}

/// Encode: substitute dictionary entries with 2-byte codon tokens.
/// Returns tokenized buffer. Prepends no header -- caller handles framing.
pub fn assemble_from_matches(buf: &[u8], matches: &[(usize, u8, usize)]) -> Vec<u8> {
    if matches.is_empty() {
        return escape_only(buf);
    }
    // Greedy left-to-right assembly
    let mut out = Vec::with_capacity(buf.len());
    let mut pos = 0usize;
    let mut mi = 0usize;

    while pos < buf.len() {
        // Advance past stale matches
        while mi < matches.len() && matches[mi].0 < pos {
            mi += 1;
        }

        if mi < matches.len() && matches[mi].0 == pos {
            let (_, token_id, match_len) = matches[mi];
            out.push(ESCAPE);
            out.push(token_id);
            pos += match_len;
            mi += 1;
        } else {
            // Copy bytes up to next match, escaping any literal ESCAPE bytes
            let next_match = if mi < matches.len() {
                matches[mi].0
            } else {
                buf.len()
            };
            let end = next_match.min(buf.len());
            let mut j = pos;
            while j < end {
                if buf[j] == ESCAPE {
                    if j > pos {
                        out.extend_from_slice(&buf[pos..j]);
                    }
                    out.push(ESCAPE);
                    out.push(0x00);
                    pos = j + 1;
                }
                j += 1;
            }
            if pos < end {
                out.extend_from_slice(&buf[pos..end]);
            }
            pos = end;
        }
    }
    out
}


pub fn encode(buf: &[u8], entries: &[DictEntry]) -> Vec<u8> {
    if entries.is_empty() {
        return escape_only(buf);
    }
    let index = FirstByteIndex::build(entries);
    let matches = index.find_matches(buf);
    assemble_from_matches(buf, &matches)
}

/// Decode: reverse codon substitution using dictionary entries.
pub fn decode(buf: &[u8], entries: &[DictEntry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() * 2);
    let mut i = 0usize;

    while i < buf.len() {
        if buf[i] == ESCAPE && i + 1 < buf.len() {
            let id = buf[i + 1];
            if id == 0x00 {
                out.push(ESCAPE); // literal escape byte
            } else {
                let idx = (id as usize).saturating_sub(1);
                if idx < entries.len() {
                    out.extend_from_slice(&entries[idx].bytes);
                } else {
                    // Unknown token -- pass through (forward compat)
                    out.push(ESCAPE);
                    out.push(id);
                }
            }
            i += 2;
        } else {
            out.push(buf[i]);
            i += 1;
        }
    }
    out
}

/// Escape literal ESCAPE bytes only -- used when dictionary is empty.
fn escape_only(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len());
    for &b in buf {
        if b == ESCAPE {
            out.push(ESCAPE);
            out.push(0x00);
        } else {
            out.push(b);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::dictionary::{self, DictEntry};

    fn entry(s: &str) -> DictEntry {
        DictEntry {
            bytes: s.as_bytes().to_vec(),
            freq: 10,
            saving: 100,
        }
    }

    #[test]
    fn test_roundtrip_empty_dict() {
        let buf = b"hello world".to_vec();
        let enc = encode(&buf, &[]);
        let dec = decode(&enc, &[]);
        assert_eq!(dec, buf);
    }

    #[test]
    fn test_roundtrip_with_dict() {
        let entries = vec![entry("hello"), entry("world")];
        let buf = b"hello world hello world".to_vec();
        let enc = encode(&buf, &entries);
        let dec = decode(&enc, &entries);
        assert_eq!(dec, buf, "roundtrip failed");
        // Encoded should be smaller
        assert!(enc.len() < buf.len(), "should compress");
    }

    #[test]
    fn test_escape_literal() {
        // Buffer containing literal ESCAPE byte must survive roundtrip
        let mut buf = b"data".to_vec();
        buf.push(ESCAPE);
        buf.extend_from_slice(b"more");
        let enc = encode(&buf, &[]);
        let dec = decode(&enc, &[]);
        assert_eq!(dec, buf, "literal ESCAPE roundtrip failed");
    }

    #[test]
    fn test_full_pipeline() {
        // Build dict from corpus, encode, decode, verify lossless
        let corpus = b"the quick brown fox jumps over the lazy dog \
                        the quick brown fox jumps over the lazy dog \
                        the quick brown fox jumps over the lazy dog"
            .to_vec();
        let entries = dictionary::build(&corpus);
        assert!(
            !entries.is_empty(),
            "should build dictionary from repetitive corpus"
        );
        let enc = encode(&corpus, &entries);
        let dec = decode(&enc, &entries);
        assert_eq!(dec, corpus, "full pipeline roundtrip failed");
        assert!(enc.len() < corpus.len(), "should achieve compression");
    }

    #[test]
    fn test_lossless_real_messages() {
        // Simulate repetitive message batch
        let msg = "user123 joined channel #general at 2024-01-01 ";
        let corpus: Vec<u8> = msg.repeat(50).into_bytes();
        let entries = dictionary::build(&corpus);
        let enc = encode(&corpus, &entries);
        let dec = decode(&enc, &entries);
        assert_eq!(dec, corpus, "message batch roundtrip failed");
    }

    #[test]
    fn test_token_collision_safety() {
        // Entries with overlapping content must not corrupt data
        let entries = vec![entry("hello world"), entry("hello"), entry("world")];
        let buf = b"hello world hello world".to_vec();
        let enc = encode(&buf, &entries);
        let dec = decode(&enc, &entries);
        assert_eq!(dec, buf);
    }
}
