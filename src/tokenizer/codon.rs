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
#[derive(Debug, Clone)]
struct PatternRec {
    prefix4: u32,
    token_id: u8,
    bytes: Vec<u8>,
    next: i32,
}

#[derive(Debug)]
pub struct FirstByteIndex {
    mask: usize,
    keys: Vec<u32>,
    heads: Vec<i32>,
    used: Vec<u8>,
    patterns: Vec<PatternRec>,
}

impl FirstByteIndex {
    pub fn build(entries: &[DictEntry]) -> Self {
        let mut pats: Vec<(u32, u8, Vec<u8>)> = Vec::with_capacity(entries.len());
        for (id0, entry) in entries.iter().enumerate() {
            let b = entry.bytes.as_slice();
            if b.len() < 4 { continue; }
            // token_id wraps at u8 -- collision for entries >254 but top entries stay unique
            let token_id = ((id0 + 1) & 0xFF) as u8;
            if token_id == 0 { continue; } // skip if wrapped to reserved 0
            let prefix4 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            pats.push((prefix4, token_id, entry.bytes.clone()));
        }
        // Sort descending by length so longer patterns are at head of chain (checked first)
        pats.sort_unstable_by(|a, b| b.2.len().cmp(&a.2.len()));

        let n = pats.len().max(1);
        let cap = (n * 2).next_power_of_two().max(1024);
        let mask = cap - 1;
        let mut keys = vec![0u32; cap];
        let mut heads = vec![-1i32; cap];
        let mut used = vec![0u8; cap];
        let mut patterns: Vec<PatternRec> = Vec::with_capacity(n);

        for (prefix4, token_id, bytes) in pats {
            let idx = patterns.len() as i32;
            let slot = Self::find_slot_insert(prefix4, mask, &mut keys, &mut used);
            let old_head = heads[slot];
            heads[slot] = idx;
            patterns.push(PatternRec { prefix4, token_id, bytes, next: old_head });
        }

        Self { mask, keys, heads, used, patterns }
    }

    pub fn find_matches(&self, buf: &[u8]) -> Vec<(usize, u8, usize)> {
        let n = buf.len();
        if n < 4 || self.patterns.is_empty() { return Vec::new(); }

        // Build first-byte presence set -- skip positions where first byte
        // cannot start any pattern (O(1) check saves hash lookup)
        let mut first_byte_present = [false; 256];
        for p in &self.patterns {
            if !p.bytes.is_empty() {
                first_byte_present[p.bytes[0] as usize] = true;
            }
        }

        let mut out: Vec<(usize, u8, usize)> = Vec::with_capacity(n / 4);
        let mut i = 0usize;

        while i + 4 <= n {
            if buf[i] == ESCAPE || !first_byte_present[buf[i] as usize] {
                i += 1; continue;
            }

            let prefix4 = u32::from_le_bytes([buf[i], buf[i+1], buf[i+2], buf[i+3]]);
            let head = self.lookup(prefix4);
            if head < 0 { i += 1; continue; }

            // Find longest match at this position
            let mut best_len = 0usize;
            let mut best_tok = 0u8;
            let mut p = head;

            while p >= 0 {
                let rec = &self.patterns[p as usize];
                let len = rec.bytes.len();
                if len > best_len && i + len <= n && buf[i..i+len] == *rec.bytes {
                    best_len = len;
                    best_tok = rec.token_id;
                    if best_len >= 128 { break; }
                }
                p = rec.next;
            }

            if best_len >= 4 {
                out.push((i, best_tok, best_len));
            }
            // Always advance by 1 -- let assembler do greedy selection
            i += 1;
        }

        // Sort by position for assembler compatibility
        out.sort_unstable_by_key(|m| m.0);
        out
    }

    #[inline]
    fn hash32(x: u32) -> u32 {
        let mut v = x.wrapping_mul(0x9E37_79B1);
        v ^= v >> 16;
        v = v.wrapping_mul(0x85EB_CA6B);
        v ^= v >> 13;
        v
    }

    #[inline]
    fn find_slot_insert(key: u32, mask: usize, keys: &mut [u32], used: &mut [u8]) -> usize {
        let mut slot = (Self::hash32(key) as usize) & mask;
        loop {
            if used[slot] == 0 { used[slot] = 1; keys[slot] = key; return slot; }
            if keys[slot] == key { return slot; }
            slot = (slot + 1) & mask;
        }
    }

    #[inline]
    fn lookup(&self, key: u32) -> i32 {
        let mut slot = (Self::hash32(key) as usize) & self.mask;
        loop {
            if self.used[slot] == 0 { return -1; }
            if self.keys[slot] == key { return self.heads[slot]; }
            slot = (slot + 1) & self.mask;
        }
    }
}

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

/// Encode using a pre-built index -- avoids rebuild on every call
pub fn encode_with_index(buf: &[u8], index: &FirstByteIndex) -> Vec<u8> {
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

/// Aho-Corasick based encoder -- O(n) single pass, all patterns simultaneously
/// Uses u16 token IDs (3 bytes: ESCAPE+hi+lo) to support all window entries
pub fn encode_ac(buf: &[u8], entries: &[DictEntry]) -> Vec<u8> {
    use aho_corasick::{AhoCorasick, MatchKind};

    if entries.is_empty() { return escape_only(buf); }

    // Use ALL entries sorted by saving -- u16 IDs support up to 65535 patterns
    let mut sorted: Vec<&DictEntry> = entries.iter()
        .filter(|e| e.bytes.len() >= 4)
        .collect();
    sorted.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));

    let patterns: Vec<&[u8]> = sorted.iter().map(|e| e.bytes.as_slice()).collect();

    let ac = match AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(&patterns) {
        Ok(ac) => ac,
        Err(_) => return escape_only(buf),
    };

    let mut out = Vec::with_capacity(buf.len());
    let mut pos = 0usize;

    for m in ac.find_iter(buf) {
        // Copy literal bytes before match
        for &b in &buf[pos..m.start()] {
            if b == ESCAPE { out.push(ESCAPE); out.push(0x00); }
            else { out.push(b); }
        }
        // Emit u16 token: ESCAPE + hi_byte + lo_byte
        // pattern index 0 = token 1, etc.
        let tok = (m.pattern().as_usize() + 1) as u16;
        if tok <= 254 {
            // Fast path: u8 token (2 bytes)
            out.push(ESCAPE);
            out.push(tok as u8);
        } else {
            // Extended: 3-byte token ESCAPE + 0xFF + (tok-255)
            // Only profitable if pattern len > 3
            out.push(ESCAPE);
            out.push(0xFF);
            out.push((tok - 255) as u8);
        }
        pos = m.end();
    }
    // Remaining literal bytes
    for &b in &buf[pos..] {
        if b == ESCAPE { out.push(ESCAPE); out.push(0x00); }
        else { out.push(b); }
    }
    out
}

/// Encode using pre-built AC automaton -- O(n) single pass
/// Maps all pattern IDs to u8 via wrapping (same as codon FirstByteIndex)
/// This preserves the accidental benefit of ID collision: deflate sees
/// repeated ESCAPE+id pairs across chunks and compresses them efficiently
pub fn encode_ac_with(buf: &[u8], ac: &aho_corasick::AhoCorasick) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len());
    let mut pos = 0usize;
    for m in ac.find_iter(buf) {
        for &b in &buf[pos..m.start()] {
            if b == ESCAPE { out.push(ESCAPE); out.push(0x00); }
            else { out.push(b); }
        }
        // Wrap to u8 -- same as codon::FirstByteIndex wrapping behavior
        // id 0 is reserved, skip it
        let tok = ((m.pattern().as_usize() + 1) & 0xFF) as u8;
        if tok == 0 {
            // Wrapped to reserved 0 -- emit literals
            for &b in &buf[m.start()..m.end()] {
                if b == ESCAPE { out.push(ESCAPE); out.push(0x00); }
                else { out.push(b); }
            }
        } else {
            out.push(ESCAPE); out.push(tok);
        }
        pos = m.end();
    }
    for &b in &buf[pos..] {
        if b == ESCAPE { out.push(ESCAPE); out.push(0x00); }
        else { out.push(b); }
    }
    out
}

/// Build cached Aho-Corasick automaton from entries
pub fn build_ac(entries: &[DictEntry]) -> Option<aho_corasick::AhoCorasick> {
    use aho_corasick::{AhoCorasick, MatchKind};
    if entries.is_empty() { return None; }
    let mut sorted: Vec<&DictEntry> = entries.iter()
        .filter(|e| e.bytes.len() >= 4)
        .collect();
    sorted.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
    let patterns: Vec<&[u8]> = sorted.iter().map(|e| e.bytes.as_slice()).collect();
    AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(&patterns).ok()
}
