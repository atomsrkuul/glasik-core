//! sliding.rs -- Sliding window codon table
//!
//! Maintains dictionary state across compress() calls.
//! Unlike per-batch dicts, this accumulates domain knowledge
//! over time -- the longer it runs on a stream, the better it gets.
//!
//! Architecture:
//!   - Fixed-size LRU entry table (max MAX_WINDOW_ENTRIES)
//!   - Each entry tracks: bytes, cumulative_freq, last_seen_batch
//!   - New patterns promoted in, stale low-freq patterns evicted
//!   - Dictionary serialized into each frame header for self-contained decode
//!
//! This is how we beat gzip: gzip resets per stream.
//! We accumulate domain vocabulary indefinitely.

use std::collections::HashMap;
use crate::tokenizer::dictionary::DictEntry;
use crate::tokenizer::dictionary::{build, build_second_pass, extract_residual};
use crate::tokenizer::codon::{encode as codon_encode, decode as codon_decode};
use crate::tokenizer::TOK_MAGIC;
use crate::tokenizer::dictionary::{serialize, deserialize};

pub const MAX_WINDOW_ENTRIES: usize = 200;
pub const EVICTION_AGE:       u64   = 50;  // evict if not seen in N batches

#[derive(Debug, Clone)]
struct WindowEntry {
    bytes:          Vec<u8>,
    cumulative_freq: u64,
    last_seen:      u64,   // batch number
    saving:         u64,
}

/// Sliding window codon table.
/// Create once, compress many batches through it.
pub struct SlidingTokenizer {
    window:      Vec<WindowEntry>,
    batch_count: u64,
}

impl SlidingTokenizer {
    pub fn new() -> Self {
        SlidingTokenizer { window: Vec::new(), batch_count: 0 }
    }

    /// Compress a buffer using accumulated window knowledge.
    /// Updates window with new patterns found in this batch.
    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        self.batch_count += 1;

        // Build batch dict from current buffer
        let batch_entries = build(buf);

        // Merge batch findings into window
        self.update_window(&batch_entries);

        // Evict stale low-value entries
        self.evict();

        // Select top entries from window for this encode
        let active = self.active_entries();

        // Two-pass encode
        let tokenized1 = codon_encode(buf, &active);
        let residual   = extract_residual(&tokenized1);
        let pass2      = build_second_pass(&residual, active.len());
        let tokenized2 = if pass2.is_empty() {
            tokenized1
        } else {
            apply_pass2_sliding(&tokenized1, &pass2, active.len())
        };

        // Merge for header
        let mut merged = active.clone();
        merged.extend(pass2);
        let dict_header = serialize(&merged);

        let mut out = Vec::with_capacity(4 + dict_header.len() + 4 + tokenized2.len());
        out.extend_from_slice(&TOK_MAGIC);
        out.extend_from_slice(&dict_header);
        out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
        out.extend_from_slice(&tokenized2);
        out
    }

    /// Decode is stateless -- dictionary is in the frame header.
    pub fn decode(buf: &[u8]) -> Result<Vec<u8>, String> {
        if buf.len() < 4 { return Err("sliding: too short".into()); }
        if buf[0..4] != TOK_MAGIC { return Ok(buf.to_vec()); }
        let (entries, dict_end) = deserialize(&buf[4..])
            .map_err(|e| format!("sliding: {e}"))?;
        let header_end = 4 + dict_end;
        if header_end + 4 > buf.len() { return Err("sliding: truncated".into()); }
        let orig_len = u32::from_le_bytes(
            buf[header_end..header_end+4].try_into().unwrap()
        ) as usize;
        let decoded = codon_decode(&buf[header_end+4..], &entries);
        if decoded.len() != orig_len {
            return Err(format!("sliding: len mismatch {} vs {}", decoded.len(), orig_len));
        }
        Ok(decoded)
    }

    /// Window stats for benchmarking
    pub fn stats(&self) -> (usize, u64) {
        (self.window.len(), self.batch_count)
    }

    // ── Private ────────────────────────────────────────────────────────

    fn update_window(&mut self, batch: &[DictEntry]) {
        let mut index: HashMap<Vec<u8>, usize> = self.window.iter().enumerate()
            .map(|(i, e)| (e.bytes.clone(), i))
            .collect();

        for entry in batch {
            if let Some(&idx) = index.get(&entry.bytes) {
                // Update existing
                self.window[idx].cumulative_freq += entry.freq as u64;
                self.window[idx].last_seen        = self.batch_count;
                self.window[idx].saving           += entry.saving as u64;
            } else if self.window.len() < MAX_WINDOW_ENTRIES {
                // Add new entry
                self.window.push(WindowEntry {
                    bytes:           entry.bytes.clone(),
                    cumulative_freq: entry.freq as u64,
                    last_seen:       self.batch_count,
                    saving:          entry.saving as u64,
                });
                index.insert(entry.bytes.clone(), self.window.len() - 1);
            } else {
                // Window full -- replace lowest-value stale entry if new is better
                let new_value = entry.saving as u64;
                if let Some(worst_idx) = self.worst_entry(new_value) {
                    self.window[worst_idx] = WindowEntry {
                        bytes:           entry.bytes.clone(),
                        cumulative_freq: entry.freq as u64,
                        last_seen:       self.batch_count,
                        saving:          entry.saving as u64,
                    };
                }
            }
        }
    }

    fn evict(&mut self) {
        self.window.retain(|e| {
            let age = self.batch_count.saturating_sub(e.last_seen);
            age < EVICTION_AGE || e.cumulative_freq > 10
        });
    }

    fn worst_entry(&self, min_value: u64) -> Option<usize> {
        self.window.iter().enumerate()
            .filter(|(_, e)| {
                let age = self.batch_count.saturating_sub(e.last_seen);
                e.saving < min_value && age > 5
            })
            .min_by_key(|(_, e)| e.saving)
            .map(|(i, _)| i)
    }

    fn active_entries(&self) -> Vec<DictEntry> {
        let mut entries: Vec<&WindowEntry> = self.window.iter().collect();
        entries.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
        entries.iter().map(|e| DictEntry {
            bytes:  e.bytes.clone(),
            freq:   e.cumulative_freq as usize,
            saving: e.saving as usize,
        }).collect()
    }
}

impl Default for SlidingTokenizer {
    fn default() -> Self { Self::new() }
}

fn apply_pass2_sliding(
    buf: &[u8],
    pass2: &[DictEntry],
    pass1_count: usize,
) -> Vec<u8> {
    use crate::tokenizer::codon::ESCAPE;
    let mut out = Vec::with_capacity(buf.len());
    let mut i   = 0usize;
    while i < buf.len() {
        if buf[i] == ESCAPE && i + 1 < buf.len() {
            out.push(buf[i]); out.push(buf[i+1]); i += 2;
        } else {
            let start = i;
            while i < buf.len() && !(buf[i] == ESCAPE && i+1 < buf.len()) { i += 1; }
            let encoded = codon_encode(&buf[start..i], pass2);
            let mut j = 0;
            while j < encoded.len() {
                if encoded[j] == ESCAPE && j+1 < encoded.len() {
                    let id = encoded[j+1];
                    out.push(ESCAPE);
                    if id == 0x00 { out.push(0x00); }
                    else { out.push(id + pass1_count as u8); }
                    j += 2;
                } else { out.push(encoded[j]); j += 1; }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sliding_roundtrip() {
        let mut tok = SlidingTokenizer::new();
        let data = b"hello glasik hello glasik hello glasik".to_vec();
        let enc = tok.encode(&data);
        let dec = SlidingTokenizer::decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn test_sliding_improves_over_batches() {
        let mut tok = SlidingTokenizer::new();
        let msg = "user joined channel general timestamp payload data ";
        let batch: Vec<u8> = msg.repeat(100).into_bytes();
        let mut sizes = Vec::new();
        for _ in 0..5 {
            let enc = tok.encode(&batch);
            sizes.push(enc.len());
            let dec = SlidingTokenizer::decode(&enc).unwrap();
            assert_eq!(dec, batch, "lossless check failed");
        }
        println!("sizes across batches: {:?}", sizes);
        let (entries, batches) = tok.stats();
        println!("window: {entries} entries after {batches} batches");
        // Should not get worse over time
        assert!(sizes.last() <= sizes.first(), "should not degrade");
    }

    #[test]
    fn test_sliding_lossless_diverse() {
        let mut tok = SlidingTokenizer::new();
        let messages = vec![
            b"the quick brown fox jumps over the lazy dog".to_vec(),
            b"hello world this is a test message for compression".to_vec(),
            b"user joined channel general at timestamp 1743744000".to_vec(),
        ];
        for msg in &messages {
            let enc = tok.encode(msg);
            let dec = SlidingTokenizer::decode(&enc).unwrap();
            assert_eq!(&dec, msg);
        }
    }
}
