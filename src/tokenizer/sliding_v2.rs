//! sliding_v2.rs -- External-dictionary sliding window tokenizer
//!
//! Key difference from v1: dictionary is NOT serialized per frame.
//! Encoder and decoder share dictionary state externally.
//! Frame format: GNSL magic + dict_version(4) + orig_len(4) + payload
//!
//! This eliminates the ~2KB per-frame dict overhead that made v1
//! net-negative on short chunks.

use crate::tokenizer::codon::{decode as codon_decode, encode as codon_encode, encode_with_index as codon_encode_with_index, FirstByteIndex};
use crate::tokenizer::dictionary::{build, DictEntry};
use ahash::AHashMap as HashMap;

pub const SLIDING_MAGIC: &[u8; 4] = b"GNSL";
pub const MAX_WINDOW_ENTRIES: usize = 20000;
pub const EVICTION_AGE: u64 = 100;
pub const MIN_SAVING_THRESHOLD: usize = 3;

#[derive(Debug, Clone)]
struct WindowEntry {
    bytes: Vec<u8>,
    cumulative_freq: u64,
    last_seen: u64,
    saving: u64,
}

#[derive(Debug)]
pub struct SlidingTokenizerV2 {
    window: Vec<WindowEntry>,
    batch_count: u64,
    dict_version: u32,
    index: HashMap<Vec<u8>, usize>,
    cached_index: Option<FirstByteIndex>,
    cached_entries: Vec<crate::tokenizer::dictionary::DictEntry>,
    cached_ac: Option<aho_corasick::AhoCorasick>,
    index_dirty: bool,
    ac_dirty: bool,
}

impl SlidingTokenizerV2 {
    pub fn new() -> Self {
        SlidingTokenizerV2 {
            window: Vec::new(),
            batch_count: 0,
            dict_version: 0,
            index: HashMap::new(),
            cached_index: None,
            cached_entries: Vec::new(),
            cached_ac: None,
            index_dirty: true,
            ac_dirty: true,
        }
    }

    /// Encode a buffer. Dictionary NOT included in output.
    /// Caller must ensure decoder has matching dict_version.
    /// Fast ingest: update window statistics without encoding
    /// Use to warm the model on prior context without paying encode cost
    pub fn ingest_fast(&mut self, buf: &[u8]) {
        let batch_entries = build(buf);
        let changed = self.update_window(&batch_entries);
        self.batch_count += 1;
        if changed {
            self.dict_version = self.dict_version.wrapping_add(1);
            self.index_dirty = self.batch_count % 10 == 0;
        }
    }

    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        self.batch_count += 1;
        let batch_entries = build(buf);
        let changed = self.update_window(&batch_entries);
        if changed {
            self.dict_version = self.dict_version.wrapping_add(1);
            self.index_dirty = true;
            if self.batch_count % 50 == 0 { self.ac_dirty = true; }
        }

        let tokenized = if self.window.is_empty() {
            buf.to_vec()
        } else {
            let index = self.get_index();
            codon_encode_with_index(buf, index)
        };
        // Frame: GNSL + dict_version(4) + orig_len(4) + payload
        let mut out = Vec::with_capacity(12 + tokenized.len());
        out.extend_from_slice(SLIDING_MAGIC);
        out.extend_from_slice(&self.dict_version.to_le_bytes());
        // Guard against inputs > u32::MAX
        let orig_len = u32::try_from(buf.len())
            .unwrap_or(u32::MAX);
        out.extend_from_slice(&orig_len.to_le_bytes());
        out.extend_from_slice(&tokenized);
        out
    }

    /// Decode using current dictionary state.
    pub fn decode(&self, buf: &[u8]) -> Result<Vec<u8>, String> {
        if buf.len() < 12 {
            return Err("sliding_v2: too short".into());
        }
        if &buf[0..4] != SLIDING_MAGIC {
            return Err("sliding_v2: bad magic".into());
        }
        let _dict_version = u32::from_le_bytes(
            buf[4..8].try_into().map_err(|_| "sliding_v2: bad dict_version".to_string())?);
        let orig_len = u32::from_le_bytes(
            buf[8..12].try_into().map_err(|_| "sliding_v2: bad orig_len".to_string())?) as usize;
        let payload = &buf[12..];

        let active = self.active_entries();
        let decoded = if active.is_empty() {
            payload.to_vec()
        } else {
            codon_decode(payload, &active)
        };

        if decoded.len() != orig_len {
            return Err(format!(
                "sliding_v2: len mismatch {} vs {}", decoded.len(), orig_len
            ));
        }
        Ok(decoded)
    }

    /// Export current dictionary for out-of-band sync or storage.
    pub fn export_dict(&self) -> (u32, Vec<(Vec<u8>, u64, u64)>) {
        let entries = self.active_entries();
        let exported = entries.iter().map(|e| {
            (e.bytes.clone(), e.freq as u64, e.saving as u64)
        }).collect();
        (self.dict_version, exported)
    }

    /// Import dictionary (for decoder side initialization).
    pub fn import_dict(&mut self, version: u32, entries: Vec<(Vec<u8>, u64, u64)>) {
        self.window.clear();
        self.index.clear();
        self.cached_index = None;
        self.cached_entries.clear();
        self.index_dirty = true;
        self.ac_dirty = true; // full vocab replacement -- AC must rebuild
        self.dict_version = version;
        for (bytes, freq, saving) in entries {
            let idx = self.window.len();
            self.index.insert(bytes.clone(), idx);
            self.window.push(WindowEntry {
                bytes,
                cumulative_freq: freq,
                last_seen: self.batch_count,
                saving,
            });
        }
    }

    pub fn dict_version(&self) -> u32 { self.dict_version }
    pub fn stats(&self) -> (usize, u64) { (self.window.len(), self.batch_count) }

    fn update_window(&mut self, batch: &[DictEntry]) -> bool {
        let mut changed = false;
        for entry in batch {
            if entry.saving < MIN_SAVING_THRESHOLD {
                continue;
            }
            if let Some(&idx) = self.index.get(&entry.bytes) {
                self.window[idx].cumulative_freq += entry.freq as u64;
                self.window[idx].last_seen = self.batch_count;
                self.window[idx].saving += entry.saving as u64;
            } else if self.window.len() < MAX_WINDOW_ENTRIES {
                let idx = self.window.len();
                self.index.insert(entry.bytes.clone(), idx);
                self.window.push(WindowEntry {
                    bytes: entry.bytes.clone(),
                    cumulative_freq: entry.freq as u64,
                    last_seen: self.batch_count,
                    saving: entry.saving as u64,
                });
                changed = true;
            } else {
                let new_value = entry.saving as u64;
                if let Some(worst_idx) = self.worst_entry(new_value) {
                    let old_bytes = self.window[worst_idx].bytes.clone();
                    self.index.remove(&old_bytes);
                    self.window[worst_idx] = WindowEntry {
                        bytes: entry.bytes.clone(),
                        cumulative_freq: entry.freq as u64,
                        last_seen: self.batch_count,
                        saving: entry.saving as u64,
                    };
                    self.index.insert(entry.bytes.clone(), worst_idx);
                    changed = true;
                }
            }
        }
        if changed {
            self.index_dirty = true;
            // AC throttled: frequency updates do not change pattern set, rebuild amortized every 50 batches
        }
        changed
    }

    fn evict(&mut self) {
        // Phase 1: no eviction -- L0/L1/L2/L3 tiered architecture eliminates
        // the need for eviction on the hot path. Window saturates at MAX_WINDOW_ENTRIES
        // and remains stable. Revisit (incremental only) if real sessions show
        // measurable stale-pattern degradation.
        if self.window.len() >= MAX_WINDOW_ENTRIES {
            eprintln!("GN warn: L2 window saturated at {} entries -- no eviction active", MAX_WINDOW_ENTRIES);
        }
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

    fn get_index(&mut self) -> &FirstByteIndex {
        if self.index_dirty {
            let mut entries: Vec<&WindowEntry> = self.window.iter().collect();
            entries.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
            self.cached_entries = entries.iter().map(|e| DictEntry {
                bytes: e.bytes.clone(),
                freq: e.cumulative_freq as usize,
                saving: e.saving as usize,
            }).collect();
            self.cached_index = Some(FirstByteIndex::build(&self.cached_entries));
            if self.ac_dirty {
                self.cached_ac = crate::tokenizer::codon::build_ac(&self.cached_entries);
                self.ac_dirty = false;
            }
            self.index_dirty = false;
        }
        self.cached_index.as_ref().expect("cached_index must be Some after get_index build")
    }

    pub fn active_entries_pub(&self) -> Vec<DictEntry> { self.active_entries() }

    /// Decode raw tokenized bytes using current window vocab (no frame header)
    /// Split-stream encode using cached AC -- returns (tok_ids, literals)
    pub fn encode_ac_split(&mut self, buf: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let _ = self.get_index();
        if let Some(ref ac) = self.cached_ac {
            crate::tokenizer::codon::encode_ac_split(buf, ac)
        } else {
            (Vec::new(), buf.to_vec())
        }
    }

    /// Decode split-stream using cached AC + vocab
    pub fn decode_ac_split(&mut self, buf: &[u8], tok_ids: &[u8], literals: &[u8]) -> Vec<u8> {
        let _ = self.get_index();
        let entries = self.active_entries();
        if let Some(ref ac) = self.cached_ac {
            crate::tokenizer::codon::decode_ac_split(buf, tok_ids, literals, ac, &entries)
        } else {
            literals.to_vec()
        }
    }

    pub fn decode_raw(&self, tokenized: &[u8]) -> Result<Vec<u8>, String> {
        let active = self.active_entries();
        if active.is_empty() { return Ok(tokenized.to_vec()); }
        Ok(crate::tokenizer::codon::decode(tokenized, &active))
    }

    /// Encode using cached Aho-Corasick automaton -- O(n) matching
    pub fn encode_ac(&mut self, buf: &[u8]) -> Vec<u8> {
        // Ensure AC is built
        let _ = self.get_index();
        if let Some(ref ac) = self.cached_ac {
            crate::tokenizer::codon::encode_ac_with(buf, ac)
        } else {
            let active = self.active_entries();
            crate::tokenizer::codon::encode_ac(buf, &active)
        }
    }

    fn active_entries(&self) -> Vec<DictEntry> {
        if !self.cached_entries.is_empty() && !self.index_dirty {
            return self.cached_entries.clone();
        }
        let mut entries: Vec<&WindowEntry> = self.window.iter().collect();
        entries.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
        entries.iter().map(|e| DictEntry {
            bytes: e.bytes.clone(),
            freq: e.cumulative_freq as usize,
            saving: e.saving as usize,
        }).collect()
    }

    /// Initialize with pre-trained static dictionary entries.
    /// Static entries get high base frequency so eviction treats them as established.
    pub fn new_with_static(entries: Vec<(Vec<u8>, u64, u64)>) -> Self {
        let mut tok = SlidingTokenizerV2::new();
        for (bytes, freq, saving) in entries {
            if saving < MIN_SAVING_THRESHOLD as u64 { continue; }
            let idx = tok.window.len();
            if idx >= MAX_WINDOW_ENTRIES { break; }
            tok.index.insert(bytes.clone(), idx);
            tok.window.push(WindowEntry {
                bytes,
                cumulative_freq: freq,
                last_seen: 0, // batch 0 = static, never evicted by age alone
                saving,
            });
        }
        tok.dict_version = 1;
        tok.index_dirty = true;
        tok.ac_dirty = true;
        tok.cached_ac = None;
        tok
    }


}

impl Default for SlidingTokenizerV2 {
    fn default() -> Self { Self::new() }
}
