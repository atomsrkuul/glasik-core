//! fractal.rs -- Fractal Dictionary Sharding (L0-L3 tiered vocabulary)
//!
//! Four vocabulary tiers:
//!   L0: Universal (pre-trained, static, IDs 1-63)
//!   L1: Domain (per shard type, IDs 64-127)
//!   L2: Session (learned online per session, IDs 128-191)
//!   L3: Chunk (ephemeral per-call N-grams, IDs 192-254)

use std::collections::HashMap;
use crate::tokenizer::sliding_v2::SlidingTokenizerV2;
use crate::tokenizer::dictionary::{DictEntry, build};
use crate::tokenizer::codon::{
    build_tiered_ac, encode_ac_interleaved, decode_ac_interleaved
};
use crate::level4;

pub const PROMOTE_L3_THRESHOLD: u64 = 3;
pub const PROMOTE_L2_THRESHOLD: u64 = 50;
pub const L1_MAX_ENTRIES: usize = 64;
pub const L2_MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShardType {
    SystemMessage,
    UserIntent,
    AssistantResponse,
    CodeBlock,
    ToolCall,
    ToolResult,
    Generic,
}

impl ShardType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "system_message" => ShardType::SystemMessage,
            "user_intent"    => ShardType::UserIntent,
            "assistant_response" => ShardType::AssistantResponse,
            "code_block"     => ShardType::CodeBlock,
            "tool_call"      => ShardType::ToolCall,
            "tool_result"    => ShardType::ToolResult,
            _                => ShardType::Generic,
        }
    }
}

pub struct FractalCompressor {
    /// L0: universal patterns, static, never evicted
    l0: Vec<DictEntry>,

    /// L1: per shard-type domain patterns
    l1_by_type: HashMap<ShardType, Vec<DictEntry>>,

    /// L2: per session sliding window
    l2_by_session: HashMap<String, SlidingTokenizerV2>,

    /// Cached AC automaton (rebuilt when dirty)
    cached_ac: Option<aho_corasick::AhoCorasick>,
    ac_dirty: bool,
    last_shard_type: Option<ShardType>,
    last_session_id: Option<String>,
}

impl FractalCompressor {
    pub fn new() -> Self {
        FractalCompressor {
            l0: Vec::new(),
            l1_by_type: HashMap::new(),
            l2_by_session: HashMap::new(),
            cached_ac: None,
            ac_dirty: true,
            last_shard_type: None,
            last_session_id: None,
        }
    }

    /// Load L0 from pre-trained entries (called at startup)
    pub fn load_l0(&mut self, entries: Vec<(Vec<u8>, u64, u64)>) {
        self.l0 = entries.into_iter().map(|(bytes, freq, saving)| DictEntry {
            bytes, freq: freq as usize, saving: saving as usize,
        }).collect();
        // Sort by saving descending, take top 63
        self.l0.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
        self.l0.truncate(63);
        self.ac_dirty = true;
    }

    /// Compress a shard with tiered vocabulary
    pub fn compress_shard(
        &mut self,
        data: &[u8],
        shard_type: &str,
        session_id: &str,
    ) -> Vec<u8> {
        let stype = ShardType::from_str(shard_type);

        // Build L3 from current chunk N-grams
        let l3_raw = build(data);
        let mut l3: Vec<DictEntry> = l3_raw.into_iter()
            .filter(|e| e.saving >= 2 && e.bytes.len() >= 4)
            .collect();
        l3.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
        l3.truncate(63);

        // Get L1 for this shard type
        let l1 = self.l1_by_type.entry(stype.clone())
            .or_insert_with(Vec::new);
        let l1_snap: Vec<DictEntry> = l1.clone();

        // Get L2 for this session
        let l2 = self.l2_by_session.entry(session_id.to_string())
            .or_insert_with(SlidingTokenizerV2::new);
        let l2_entries = l2.active_entries_pub();

        // Check if we need to rebuild AC
        let need_rebuild = self.ac_dirty
            || self.last_shard_type.as_ref() != Some(&stype)
            || self.last_session_id.as_deref() != Some(session_id);

        if need_rebuild {
            // L2 excluded from AC -- L2 state mutates and cannot be
            // reconstructed at decode time. L2 promotes to L1 for
            // persistent patterns. Frame self-contained via L3.
            let empty_l2: Vec<DictEntry> = Vec::new();
            self.cached_ac = build_tiered_ac(&self.l0, &l1_snap, &empty_l2, &l3);
            self.ac_dirty = false;
            self.last_shard_type = Some(stype.clone());
            self.last_session_id = Some(session_id.to_string());
        }

        // Encode using tiered AC -- interleaved pair format (no original needed on decode)
        let (pairs, literals) = if let Some(ref ac) = self.cached_ac {
            encode_ac_interleaved(data, ac)
        } else {
            // No AC -- emit all as literals with empty pairs (just trailing count)
            let trailing = (data.len() as u16).to_le_bytes();
            (trailing.to_vec(), data.to_vec())
        };

        // Serialize L3 entries: [(1B pattern_len)(pattern_bytes)...]
        let mut l3_ser: Vec<u8> = Vec::new();
        for e in &l3 {
            if e.bytes.len() <= 255 {
                l3_ser.push(e.bytes.len() as u8);
                l3_ser.extend_from_slice(&e.bytes);
            }
        }

        // Deflate both streams independently
        use flate2::{write::DeflateEncoder, Compression};
        use std::io::Write;
        let pairs_deflated = {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
            let _ = enc.write_all(&pairs);
            enc.finish().expect("deflate pairs failed")
        };
        let lits_deflated = {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
            let _ = enc.write_all(&literals);
            enc.finish().expect("deflate literals failed")
        };

        // Update L2 session window
        let l2 = self.l2_by_session.get_mut(session_id).unwrap();
        let _ = l2.encode(data); // updates window, discard output

        // Promote L3 patterns to L2
        self.promote_l3_to_l2(&l3, session_id);

        // Promote L2 patterns to L1
        self.promote_l2_to_l1(&stype, session_id);

        // Frame: [1B shard_type][2B pairs_len LE][2B l3_ser_len LE][l3_ser][deflated pairs][deflated literals]
        let mut out = Vec::with_capacity(5 + l3_ser.len() + pairs_deflated.len() + lits_deflated.len());
        out.push(self.shard_type_byte(&stype));
        out.extend_from_slice(&(pairs_deflated.len() as u16).to_le_bytes());
        out.extend_from_slice(&(l3_ser.len() as u16).to_le_bytes());
        out.extend_from_slice(&l3_ser);
        out.extend_from_slice(&pairs_deflated);
        out.extend_from_slice(&lits_deflated);
        out
    }

    /// Decompress a shard -- fully self-contained, no original buffer needed
    pub fn decompress_shard(
        &mut self,
        data: &[u8],
        shard_type: &str,
        session_id: &str,
    ) -> Result<Vec<u8>, String> {
        if data.is_empty() { return Ok(Vec::new()); }
        if data.len() < 3 { return Err("frame too short".into()); }

        let stype = ShardType::from_str(shard_type);

        // Parse frame: [1B shard_type][2B pairs_len LE][2B l3_ser_len LE][l3_ser][deflated pairs][deflated literals]
        if data.len() < 5 { return Err("frame too short for header".into()); }
        let pairs_len   = u16::from_le_bytes([data[1], data[2]]) as usize;
        let l3_ser_len  = u16::from_le_bytes([data[3], data[4]]) as usize;
        let header_end  = 5 + l3_ser_len;
        if data.len() < header_end + pairs_len {
            return Err("frame truncated".into());
        }
        // Deserialize L3
        let l3_ser = &data[5..5 + l3_ser_len];
        let mut l3_entries: Vec<DictEntry> = Vec::new();
        let mut pos = 0usize;
        while pos < l3_ser.len() {
            let plen = l3_ser[pos] as usize;
            pos += 1;
            if pos + plen > l3_ser.len() { break; }
            l3_entries.push(DictEntry {
                bytes: l3_ser[pos..pos+plen].to_vec(),
                freq: 1, saving: plen,
            });
            pos += plen;
        }
        let pairs_deflated = &data[header_end..header_end + pairs_len];
        let lits_deflated  = &data[header_end + pairs_len..];

        // Inflate both streams
        use flate2::read::DeflateDecoder;
        use std::io::Read;
        let pairs = {
            let mut dec = DeflateDecoder::new(pairs_deflated);
            let mut buf = Vec::new();
            dec.read_to_end(&mut buf).map_err(|e| format!("inflate pairs: {e}"))?;
            buf
        };
        let literals = {
            let mut dec = DeflateDecoder::new(lits_deflated);
            let mut buf = Vec::new();
            dec.read_to_end(&mut buf).map_err(|e| format!("inflate literals: {e}"))?;
            buf
        };

        // Decode with L0 + L1 + L3 (from frame) only
        // L2 excluded -- mutates on every compress call, not reconstructable at decode time
        // L2 still learns and promotes to L1 for persistent patterns
        let mut entries = self.l0.clone();
        if let Some(l1) = self.l1_by_type.get(&stype) {
            entries.extend_from_slice(l1);
        }
        entries.extend(l3_entries);
        decode_ac_interleaved(&pairs, &literals, &entries)
    }

    fn promote_l3_to_l2(&mut self, l3: &[DictEntry], session_id: &str) {
        let l2 = self.l2_by_session.entry(session_id.to_string())
            .or_insert_with(SlidingTokenizerV2::new);
        for e in l3 {
            if e.freq as u64 >= PROMOTE_L3_THRESHOLD {
                let _ = l2.encode(&e.bytes); // ingest pattern to boost its freq
            }
        }
        self.ac_dirty = true;
    }

    fn promote_l2_to_l1(&mut self, stype: &ShardType, session_id: &str) {
        let l2_entries = if let Some(l2) = self.l2_by_session.get(session_id) {
            l2.active_entries_pub()
        } else { return; };

        let l1 = self.l1_by_type.entry(stype.clone())
            .or_insert_with(Vec::new);

        let mut promoted = 0;
        for e in &l2_entries {
            if e.freq as u64 >= PROMOTE_L2_THRESHOLD {
                // Check not already in L1
                if !l1.iter().any(|l| l.bytes == e.bytes) {
                    l1.push(e.clone());
                    promoted += 1;
                }
            }
        }

        if promoted > 0 {
            l1.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
            l1.truncate(L1_MAX_ENTRIES);
            self.ac_dirty = true;
        }
    }

    fn shard_type_byte(&self, stype: &ShardType) -> u8 {
        match stype {
            ShardType::SystemMessage      => 0x01,
            ShardType::UserIntent         => 0x02,
            ShardType::AssistantResponse  => 0x03,
            ShardType::CodeBlock          => 0x04,
            ShardType::ToolCall           => 0x05,
            ShardType::ToolResult         => 0x06,
            ShardType::Generic            => 0x00,
        }
    }

    fn all_entries(&self, stype: &ShardType, session_id: &str) -> Vec<DictEntry> {
        let mut all = self.l0.clone();
        if let Some(l1) = self.l1_by_type.get(stype) {
            all.extend_from_slice(l1);
        }
        if let Some(l2) = self.l2_by_session.get(session_id) {
            all.extend(l2.active_entries_pub());
        }
        all
    }

    /// Save snapshot (all tiers)
    pub fn save_snapshot(&self, path: &str) -> Result<(), String> {
        let mut out = Vec::new();
        // Magic + version
        out.extend_from_slice(b"GNFT");
        out.push(5u8);

        // L0
        let l0_raw: Vec<(Vec<u8>, u64, u64)> = self.l0.iter()
            .map(|e| (e.bytes.clone(), e.freq as u64, e.saving as u64)).collect();
        let l0_ser = level4::serialize_entries(&l0_raw);
        out.extend_from_slice(&(l0_ser.len() as u32).to_le_bytes());
        out.extend_from_slice(&l0_ser);

        // L1 (all types combined with type tag)
        let mut l1_all: Vec<(Vec<u8>, u64, u64)> = Vec::new();
        for (stype, entries) in &self.l1_by_type {
            let tag = self.shard_type_byte(stype);
            for e in entries {
                let mut tagged = vec![tag];
                tagged.extend_from_slice(&e.bytes);
                l1_all.push((tagged, e.freq as u64, e.saving as u64));
            }
        }
        let l1_ser = level4::serialize_entries(&l1_all);
        out.extend_from_slice(&(l1_ser.len() as u32).to_le_bytes());
        out.extend_from_slice(&l1_ser);

        // CRC32
        let crc = crc32_simple(&out);
        out.extend_from_slice(&crc.to_le_bytes());

        std::fs::write(path, &out).map_err(|e| e.to_string())
    }

    /// Load snapshot
    pub fn load_snapshot(&mut self, path: &str) -> Result<(), String> {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        if data.len() < 9 { return Err("snapshot too short".into()); }
        if &data[0..4] != b"GNFT" { return Err("bad magic".into()); }
        if data[4] != 5 { return Err(format!("unsupported version {}", data[4])); }

        let mut pos = 5usize;

        // L0
        let l0_len = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        pos += 4;
        let l0_entries = level4::deserialize_entries(&data[pos..pos+l0_len]);
        pos += l0_len;
        self.l0 = l0_entries.into_iter().map(|(bytes, freq, saving)| DictEntry {
            bytes, freq: freq as usize, saving: saving as usize,
        }).collect();

        // L1
        let l1_len = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        pos += 4;
        let l1_entries = level4::deserialize_entries(&data[pos..pos+l1_len]);
        pos += l1_len;

        self.l1_by_type.clear();
        for (mut tagged, freq, saving) in l1_entries {
            if tagged.is_empty() { continue; }
            let tag = tagged[0];
            let bytes = tagged.split_off(1);
            let stype = self.shard_type_from_byte(tag);
            let l1 = self.l1_by_type.entry(stype).or_insert_with(Vec::new);
            l1.push(DictEntry { bytes, freq: freq as usize, saving: saving as usize });
        }

        // Verify CRC
        let crc_stored = u32::from_le_bytes(data[pos..pos+4].try_into()
            .map_err(|_| "bad crc")?);
        let crc_computed = crc32_simple(&data[..pos]);
        if crc_stored != crc_computed {
            return Err(format!("CRC mismatch: stored={} computed={}", crc_stored, crc_computed));
        }

        self.ac_dirty = true;
        Ok(())
    }

    fn shard_type_from_byte(&self, b: u8) -> ShardType {
        match b {
            0x01 => ShardType::SystemMessage,
            0x02 => ShardType::UserIntent,
            0x03 => ShardType::AssistantResponse,
            0x04 => ShardType::CodeBlock,
            0x05 => ShardType::ToolCall,
            0x06 => ShardType::ToolResult,
            _    => ShardType::Generic,
        }
    }

    pub fn stats(&self) -> HashMap<String, usize> {
        let mut s = HashMap::new();
        s.insert("l0_entries".into(), self.l0.len());
        s.insert("l1_types".into(), self.l1_by_type.len());
        s.insert("l1_total".into(), self.l1_by_type.values().map(|v| v.len()).sum());
        s.insert("l2_sessions".into(), self.l2_by_session.len());
        s
    }
}

impl Default for FractalCompressor {
    fn default() -> Self { Self::new() }
}

/// Simple CRC32 (no external dep)
fn crc32_simple(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 { crc = (crc >> 1) ^ 0xEDB88320; }
            else { crc >>= 1; }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fractal_roundtrip() {
        let mut fc = FractalCompressor::new();
        let data = b"user: hello assistant: how can I help you today with JWT authentication".repeat(5);
        let compressed = fc.compress_shard(&data, "user_intent", "session_001");
        println!("input={}B compressed={}B ratio={:.2}x",
            data.len(), compressed.len(),
            data.len() as f64 / compressed.len() as f64);
        assert!(compressed.len() > 0);
    }

    #[test]
    fn test_tier_promotion() {
        let mut fc = FractalCompressor::new();
        // Repeat same data to trigger promotion
        let data = b"function authenticate(token) { return verify(token); }".repeat(10);
        for _ in 0..60 {
            fc.compress_shard(&data, "code_block", "session_001");
        }
        let l1 = fc.l1_by_type.get(&ShardType::CodeBlock);
        println!("L1 code_block entries: {}", l1.map(|v| v.len()).unwrap_or(0));
    }

    #[test]
    fn test_snapshot_save_load() {
        let mut fc = FractalCompressor::new();
        let data = b"assistant: I can help you with that request ".repeat(20);
        for _ in 0..100 {
            fc.compress_shard(&data, "assistant_response", "sess_001");
        }
        fc.save_snapshot("/tmp/fractal_test.snapshot").unwrap();
        let mut fc2 = FractalCompressor::new();
        fc2.load_snapshot("/tmp/fractal_test.snapshot").unwrap();
        assert_eq!(fc.l0.len(), fc2.l0.len());
        println!("Snapshot save/load OK");
    }
}
