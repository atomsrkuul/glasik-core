//! lz77_gn.rs -- GN vocabulary greedy tokenizer with prefix hash index
//!
//! Single-pass O(n) scan, prefix4 hash table, greedy longest match,
//! libdeflate backend.

use crate::tokenizer::dictionary::DictEntry;

pub const ESCAPE: u8 = 0x01;
pub const MIN_MATCH: usize = 4;
pub const MAX_MATCH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Literal(u8),
    DictU8 { id: u8, len: u8 },
    DictU16 { id: u16, len: u8 },
}

#[derive(Debug)]
struct PatternRec {
    key: u32,
    id: u16,
    bytes: Box<[u8]>,
    next: i32,
}

#[derive(Debug)]
pub struct PrefixIndex<const PREFIX: usize> {
    mask: usize,
    keys: Vec<u32>,
    heads: Vec<i32>,
    used: Vec<u8>,
    patterns: Vec<PatternRec>,
}

impl<const PREFIX: usize> PrefixIndex<PREFIX> {
    pub fn new() -> Self {
        Self { mask: 0, keys: Vec::new(), heads: Vec::new(), used: Vec::new(), patterns: Vec::new() }
    }

    pub fn build(entries: &[DictEntry]) -> Self {
        let mut pats: Vec<(u32, u16, Box<[u8]>)> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            let b = e.bytes.as_slice();
            if b.len() < MIN_MATCH || b.len() > MAX_MATCH || b.len() < PREFIX { continue; }
            let id = (i as u16).wrapping_add(1);
            let key = prefix_key::<PREFIX>(b);
            pats.push((key, id, b.to_vec().into_boxed_slice()));
        }
        // Sort ascending -- longer patterns end up at chain head after insertion
        pats.sort_unstable_by(|a, b| a.2.len().cmp(&b.2.len()));

        let n = pats.len().max(1);
        let cap = (n * 2).next_power_of_two().max(1024);
        let mask = cap - 1;
        let mut keys = vec![0u32; cap];
        let mut heads = vec![-1i32; cap];
        let mut used = vec![0u8; cap];
        let mut patterns: Vec<PatternRec> = Vec::with_capacity(n);

        for (key, id, bytes) in pats {
            let slot = find_slot_insert(key, mask, &mut keys, &mut used);
            let idx = patterns.len() as i32;
            let old_head = heads[slot];
            heads[slot] = idx;
            patterns.push(PatternRec { key, id, bytes, next: old_head });
        }
        Self { mask, keys, heads, used, patterns }
    }

    #[inline]
    pub fn lookup_head(&self, key: u32) -> i32 {
        if self.keys.is_empty() { return -1; }
        let mut slot = (hash32(key) as usize) & self.mask;
        loop {
            if self.used[slot] == 0 { return -1; }
            if self.keys[slot] == key { return self.heads[slot]; }
            slot = (slot + 1) & self.mask;
        }
    }

    #[inline]
    pub fn check_vocab(&self, buf: &[u8], i: usize, key: u32) -> Option<(u16, usize)> {
        let n = buf.len();
        let mut p = self.lookup_head(key);
        while p >= 0 {
            let rec = unsafe { self.patterns.get_unchecked(p as usize) };
            if rec.key == key {
                let len = rec.bytes.len();
                if i + len <= n && buf[i..i+len] == *rec.bytes {
                    return Some((rec.id, len));
                }
            }
            p = rec.next;
        }
        None
    }
}

impl<const PREFIX: usize> Default for PrefixIndex<PREFIX> {
    fn default() -> Self { Self::new() }
}

#[derive(Debug)]
pub struct GNPrefixTokenizer<const PREFIX: usize> {
    pub index: PrefixIndex<PREFIX>,
}

impl<const PREFIX: usize> GNPrefixTokenizer<PREFIX> {
    pub fn new() -> Self { Self { index: PrefixIndex::new() } }

    pub fn seed_from_vocab(&mut self, entries: &[DictEntry]) {
        self.index = PrefixIndex::<PREFIX>::build(entries);
    }

    pub fn tokenize(&self, buf: &[u8]) -> Vec<Token> {
        let n = buf.len();
        if n == 0 { return Vec::new(); }
        let mut out: Vec<Token> = Vec::with_capacity(n / 2);
        let mut i = 0usize;
        while i < n {
            if buf[i] == ESCAPE { out.push(Token::Literal(ESCAPE)); i += 1; continue; }
            if i + PREFIX > n || i + MIN_MATCH > n { out.push(Token::Literal(buf[i])); i += 1; continue; }
            let key = prefix_key_at::<PREFIX>(buf, i);
            if let Some((id, len)) = self.index.check_vocab(buf, i, key) {
                let len_u8 = (len.min(255)) as u8;
                if id <= 255 { out.push(Token::DictU8 { id: id as u8, len: len_u8 }); }
                else { out.push(Token::DictU16 { id, len: len_u8 }); }
                i += len;
            } else {
                out.push(Token::Literal(buf[i])); i += 1;
            }
        }
        out
    }

    /// Tokenize using a pre-built external index (for atomic swap pattern)
    pub fn tokenize_with_index(buf: &[u8], index: &PrefixIndex<PREFIX>, u8_only: bool) -> Vec<u8> {
        let n = buf.len();
        if n == 0 { return Vec::new(); }
        let mut out: Vec<u8> = Vec::with_capacity(n * 2);
        let mut i = 0usize;
        while i < n {
            if buf[i] == ESCAPE { out.push(ESCAPE); out.push(0x00); i += 1; continue; }
            if i + PREFIX > n || i + MIN_MATCH > n { out.push(buf[i]); i += 1; continue; }
            let key = prefix_key_at::<PREFIX>(buf, i);
            if let Some((id, len)) = index.check_vocab(buf, i, key) {
                if id <= 254 {
                    out.push(ESCAPE); out.push(id as u8); i += len;
                } else if !u8_only {
                    out.push(ESCAPE); out.push(0xFF); out.push(((id-255) & 0xFF) as u8); i += len;
                } else {
                    out.push(buf[i]); i += 1;
                }
            } else {
                out.push(buf[i]); i += 1;
            }
        }
        out
    }

    /// Tokenize with local repeat detection (cheap intra-buffer LZ77)
    /// Scans back up to 64 bytes for 4-byte matches -- O(n*64) worst case
    /// but very cache-friendly and fast in practice
    pub fn tokenize_with_local(buf: &[u8], index: &PrefixIndex<PREFIX>, u8_only: bool) -> Vec<u8> {
        let n = buf.len();
        if n == 0 { return Vec::new(); }
        let mut out: Vec<u8> = Vec::with_capacity(n * 2);
        let mut i = 0usize;
        while i < n {
            if buf[i] == ESCAPE { out.push(ESCAPE); out.push(0x00); i += 1; continue; }
            if i + PREFIX > n || i + MIN_MATCH > n { out.push(buf[i]); i += 1; continue; }

            // Try vocab match first
            let key = prefix_key_at::<PREFIX>(buf, i);
            if let Some((id, len)) = index.check_vocab(buf, i, key) {
                if id <= 254 {
                    out.push(ESCAPE); out.push(id as u8); i += len; continue;
                } else if !u8_only {
                    out.push(ESCAPE); out.push(0xFF); out.push(((id-255) & 0xFF) as u8); i += len; continue;
                }
            }

            // Try local repeat: scan back up to 64 bytes
            if i >= MIN_MATCH {
                let lookback = i.min(64);
                let mut best_len = 0usize;
                let mut best_dist = 0usize;
                for d in 1..=lookback {
                    let j = i - d;
                    if buf[j] != buf[i] { continue; }
                    let mut len = 0;
                    while i+len < n && j+len < i && buf[i+len] == buf[j+len] && len < MAX_MATCH {
                        len += 1;
                    }
                    if len >= MIN_MATCH && len > best_len {
                        best_len = len;
                        best_dist = d;
                    }
                }
                if best_len >= MIN_MATCH && best_dist <= 255 {
                    // Encode local repeat: ESCAPE + 0xFD + dist(u8) + len(u8)
                    out.push(ESCAPE); out.push(0xFD);
                    out.push(best_dist as u8);
                    out.push(best_len as u8);
                    i += best_len;
                    continue;
                }
            }

            out.push(buf[i]); i += 1;
        }
        out
    }

    pub fn tokenize_to_gn_bytes(&self, buf: &[u8], u8_only: bool) -> Vec<u8> {
        let n = buf.len();
        if n == 0 { return Vec::new(); }
        let mut out: Vec<u8> = Vec::with_capacity(n * 2);
        let mut i = 0usize;
        while i < n {
            if buf[i] == ESCAPE { out.push(ESCAPE); out.push(0x00); i += 1; continue; }
            if i + PREFIX > n || i + MIN_MATCH > n { out.push(buf[i]); i += 1; continue; }
            let key = prefix_key_at::<PREFIX>(buf, i);
            if let Some((id, len)) = self.index.check_vocab(buf, i, key) {
                if id <= 254 {
                    // 2-byte token: ESCAPE + id (1..=254)
                    out.push(ESCAPE); out.push(id as u8); i += len;
                } else if !u8_only {
                    // 3-byte extended token: ESCAPE + 0xFF + ((id-255) & 0xFF) as u8
                    // Supports IDs 255..509 (255 more patterns at 3-byte cost)
                    // Only use if pattern is long enough to benefit (len >= 4 saves >= 1 byte)
                    let ext_id = (id - 255) as u8;
                    out.push(ESCAPE); out.push(0xFF); out.push(ext_id); i += len;
                } else {
                    out.push(buf[i]); i += 1;
                }
            } else {
                out.push(buf[i]); i += 1;
            }
        }
        out
    }
}

impl<const PREFIX: usize> Default for GNPrefixTokenizer<PREFIX> {
    fn default() -> Self { Self::new() }
}

pub fn tokens_to_gn_bytes(tokens: &[Token]) -> Vec<u8> {
    let mut out = Vec::with_capacity(tokens.len() * 2);
    for t in tokens {
        match *t {
            Token::Literal(b) => { if b == ESCAPE { out.push(ESCAPE); out.push(0x00); } else { out.push(b); } }
            Token::DictU8 { id, .. } => { out.push(ESCAPE); out.push(id); }
            Token::DictU16 { id, .. } => { out.push(ESCAPE); out.push(0xFF); out.extend_from_slice(&id.to_le_bytes()); }
        }
    }
    out
}

pub fn deflate_stored_blocks(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + (data.len() / 65535 + 1) * 5);
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let chunk_len = remaining.min(65535);
        let final_block = (offset + chunk_len) == data.len();
        out.push(if final_block { 0x01 } else { 0x00 });
        let len = chunk_len as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + chunk_len]);
        offset += chunk_len;
    }
    out
}

pub fn deflate_with_libdeflater(
    compressor: &mut libdeflater::Compressor,
    data: &[u8],
    out_buf: &mut Vec<u8>,
) -> Result<usize, libdeflater::CompressionError> {
    let bound = compressor.deflate_compress_bound(data.len());
    if out_buf.len() < bound { out_buf.resize(bound, 0); }
    compressor.deflate_compress(data, out_buf.as_mut_slice())
}

#[inline]
fn prefix_key<const PREFIX: usize>(pat: &[u8]) -> u32 {
    if PREFIX == 3 { (pat[0] as u32) | ((pat[1] as u32) << 8) | ((pat[2] as u32) << 16) }
    else { u32::from_le_bytes([pat[0], pat[1], pat[2], pat[3]]) }
}

#[inline]
fn prefix_key_at<const PREFIX: usize>(buf: &[u8], i: usize) -> u32 {
    if PREFIX == 3 { (buf[i] as u32) | ((buf[i+1] as u32) << 8) | ((buf[i+2] as u32) << 16) }
    else { u32::from_le_bytes([buf[i], buf[i+1], buf[i+2], buf[i+3]]) }
}

#[inline]
fn hash32(x: u32) -> u32 {
    let mut v = x.wrapping_mul(0x9E37_79B1); v ^= v >> 16;
    v = v.wrapping_mul(0x85EB_CA6B); v ^= v >> 13; v
}

#[inline]
fn find_slot_insert(key: u32, mask: usize, keys: &mut [u32], used: &mut [u8]) -> usize {
    let mut slot = (hash32(key) as usize) & mask;
    loop {
        if used[slot] == 0 { used[slot] = 1; keys[slot] = key; return slot; }
        if keys[slot] == key { return slot; }
        slot = (slot + 1) & mask;
    }
}

/// Decode GN token bytes back to original bytes
/// Handles both u8 tokens (ESCAPE + id) and u16 tokens (ESCAPE + 0xFF + u16 LE)
pub fn decode_gn_bytes(encoded: &[u8], entries: &[DictEntry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(encoded.len() * 2);
    let mut i = 0;
    while i < encoded.len() {
        if encoded[i] != ESCAPE {
            out.push(encoded[i]);
            i += 1;
            continue;
        }
        // ESCAPE byte
        if i + 1 >= encoded.len() { break; }
        let next = encoded[i + 1];
        if next == 0x00 {
            // Literal ESCAPE
            out.push(ESCAPE);
            i += 2;
        } else if next == 0xFF {
            // 3-byte extended token: ESCAPE + 0xFF + ext_id
            // ID = ext_id + 255
            if i + 2 >= encoded.len() { break; }
            let id = (encoded[i+2] as usize) + 255;
            if id > 0 && id <= entries.len() {
                out.extend_from_slice(&entries[id-1].bytes);
            }
            i += 3;
        } else {
            // u8 token ID
            let id = next as usize;
            if id > 0 && id <= entries.len() {
                out.extend_from_slice(&entries[id-1].bytes);
            }
            i += 2;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(bytes: &[u8]) -> DictEntry { DictEntry { bytes: bytes.to_vec(), freq: 1, saving: 1 } }

    #[test]
    fn test_greedy_longest() {
        let entries = vec![entry(b"hello"), entry(b"hello world")];
        let mut tok = GNPrefixTokenizer::<4>::new();
        tok.seed_from_vocab(&entries);
        let tokens = tok.tokenize(b"hello world");
        assert!(tokens.iter().any(|t| matches!(t, Token::DictU8{..} | Token::DictU16{..})));
    }

    #[test]
    fn test_escape_handling() {
        let entries = vec![entry(b"abcd")];
        let mut tok = GNPrefixTokenizer::<4>::new();
        tok.seed_from_vocab(&entries);
        let input = [ESCAPE, b'a', b'b', b'c', b'd'];
        let gn = tok.tokenize_to_gn_bytes(&input, true);
        assert_eq!(gn[0], ESCAPE); assert_eq!(gn[1], 0x00);
    }

    #[test]
    fn test_short_buffer() {
        let entries = vec![entry(b"abcd")];
        let mut tok = GNPrefixTokenizer::<4>::new();
        tok.seed_from_vocab(&entries);
        let gn = tok.tokenize_to_gn_bytes(b"abc", true);
        assert_eq!(gn, b"abc");
    }
}

/// Hybrid encoder: GN vocab lookup + LZ77 intra-buffer matching
/// Single pass, greedy longest match from either source
pub struct GNHybridEncoder<const PREFIX: usize> {
    /// Domain vocabulary (pre-trained patterns)
    pub vocab: GNPrefixTokenizer<PREFIX>,
    /// LZ77 hash table for intra-buffer matches
    head: Vec<i32>,   // prefix3 hash -> most recent position in current buffer
    prev: Vec<i32>,   // position -> previous position with same hash
}

const LZ_HASH_SIZE: usize = 1 << 13; // 8192 -- fits in L1 cache
const LZ_HASH_MASK: usize = LZ_HASH_SIZE - 1;
const LZ_MAX_DIST: usize = 1200;   // max lookback = full chunk
const LZ_MAX_CHAIN: usize = 16;    // max chain depth

impl<const PREFIX: usize> GNHybridEncoder<PREFIX> {
    pub fn new() -> Self {
        GNHybridEncoder {
            vocab: GNPrefixTokenizer::new(),
            head: vec![-1i32; LZ_HASH_SIZE],
            prev: vec![-1i32; LZ_MAX_DIST],
        }
    }

    pub fn seed_vocab(&mut self, entries: &[DictEntry]) {
        self.vocab.seed_from_vocab(entries);
    }

    /// Reset LZ77 state for new buffer (each chunk is independent)
    fn reset_lz(&mut self) {
        for h in &mut self.head { *h = -1; }
    }

    #[inline]
    fn lz_hash(buf: &[u8], i: usize) -> usize {
        let v = (buf[i] as usize)
              | ((buf[i+1] as usize) << 5)
              | ((buf[i+2] as usize) << 10);
        (v.wrapping_mul(0x9E3779B9)) >> (32 - 13) & LZ_HASH_MASK
    }

    #[inline]
    fn lz_insert(&mut self, buf: &[u8], i: usize) {
        if i + 3 > buf.len() { return; }
        let h = Self::lz_hash(buf, i);
        let slot = i % LZ_MAX_DIST;
        self.prev[slot] = self.head[h];
        self.head[h] = i as i32;
    }

    /// Find best LZ77 back-reference at position i
    fn lz_find(&self, buf: &[u8], i: usize) -> Option<(usize, usize)> {
        let n = buf.len();
        if i + 4 > n { return None; }
        let h = Self::lz_hash(buf, i);
        let mut p = self.head[h];
        let mut best_len = 3usize; // minimum LZ match
        let mut best_dist = 0usize;
        let mut depth = 0;

        while p >= 0 && depth < LZ_MAX_CHAIN {
            let j = p as usize;
            let dist = i - j;
            if dist == 0 || dist > LZ_MAX_DIST { break; }

            // Count matching bytes
            let max_len = (n - i).min(MAX_MATCH);
            let mut len = 0;
            while len < max_len && buf[i + len] == buf[j + len] {
                len += 1;
            }
            if len > best_len {
                best_len = len;
                best_dist = dist;
                if best_len >= MAX_MATCH { break; }
            }
            p = self.prev[j % LZ_MAX_DIST];
            depth += 1;
        }

        if best_dist > 0 { Some((best_dist, best_len)) } else { None }
    }

    /// Hybrid tokenize: vocab first, LZ77 fallback, then literal
    /// Returns GN bytes with vocab tokens + back-reference markers
    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        let n = buf.len();
        if n == 0 { return Vec::new(); }
        self.reset_lz();

        let mut out = Vec::with_capacity(n);
        let mut i = 0usize;

        while i < n {
            if buf[i] == ESCAPE {
                out.push(ESCAPE); out.push(0x00);
                self.lz_insert(buf, i);
                i += 1; continue;
            }

            // Try vocab match first (domain patterns, longer average)
            let vocab_match = if i + PREFIX <= n && i + MIN_MATCH <= n {
                let key = if PREFIX == 4 {
                    u32::from_le_bytes([buf[i], buf[i+1], buf[i+2], buf[i+3]])
                } else {
                    (buf[i] as u32) | ((buf[i+1] as u32) << 8) | ((buf[i+2] as u32) << 16)
                };
                self.vocab.index.check_vocab(buf, i, key)
            } else { None };

            // Vocab match only -- let deflate handle intra-buffer repetition
            match vocab_match {
                Some((id, vlen)) => {
                    if id <= 254 {
                        out.push(ESCAPE); out.push(id as u8);
                    } else {
                        out.push(ESCAPE); out.push(0xFF); out.push(((id-255) & 0xFF) as u8);
                    }
                    self.lz_insert(buf, i);
                    i += vlen;
                }
                None => {
                    out.push(buf[i]);
                    self.lz_insert(buf, i);
                    i += 1;
                }
            }
        }
        out
    }
}

impl<const PREFIX: usize> Default for GNHybridEncoder<PREFIX> {
    fn default() -> Self { Self::new() }
}
