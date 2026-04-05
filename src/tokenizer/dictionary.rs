//! dictionary.rs -- Frequency analysis and dictionary selection
//!
//! Rolling hash (Rabin-Karp, Mersenne prime) O(n) frequency analysis.
//! Multi-pass: second dict built from non-token residual only.
//! Pattern-agnostic: structure emerges from frequency data.

pub const MIN_STR_LEN:  usize = 4;
pub const MAX_STR_LEN:  usize = 128;
pub const MIN_FREQ:     usize = 3;
pub const MAX_ENTRIES:  usize = 200;
pub const TOKEN_BYTES:  usize = 2;
pub const MAX_LENGTHS:  usize = 24;

const BASE: u64 = 131;
const MOD:  u64 = (1 << 61) - 1;

#[derive(Debug, Clone)]
pub struct DictEntry {
    pub bytes:  Vec<u8>,
    pub freq:   usize,
    pub saving: usize,
}

impl DictEntry {
    pub fn len(&self) -> usize { self.bytes.len() }
}

fn scan_length(
    buf: &[u8],
    len: usize,
    out: &mut std::collections::HashMap<Vec<u8>, usize>,
) {
    if buf.len() < len { return; }
    let mut base_pow: u64 = 1;
    for _ in 0..len {
        base_pow = base_pow.wrapping_mul(BASE) % MOD;
    }
    let mut map: std::collections::HashMap<u64, (u32, usize)> =
        std::collections::HashMap::with_capacity(buf.len() / len + 1);
    let mut hash: u64 = 0;
    for i in 0..len {
        hash = (hash.wrapping_mul(BASE) + buf[i] as u64) % MOD;
    }
    if buf[0] >= 0x20 || buf[0] == b'\n' {
        map.entry(hash).and_modify(|(c,_)| *c += 1).or_insert((1, 0));
    }
    for i in 1..=buf.len() - len {
        hash = (
            hash.wrapping_mul(BASE)
                .wrapping_add(buf[i + len - 1] as u64)
                .wrapping_add(MOD * 2)
                .wrapping_sub(base_pow.wrapping_mul(buf[i - 1] as u64) % MOD)
        ) % MOD;
        let b = buf[i];
        if b >= 0x20 || b == b'\n' || b == b'\r' {
            map.entry(hash)
               .and_modify(|(c,_)| *c += 1)
               .or_insert((1, i));
        }
    }
    for (_, (count, pos)) in map {
        if (count as usize) < MIN_FREQ { continue; }
        if pos + len > buf.len() { continue; }
        let bytes = buf[pos..pos + len].to_vec();
        let e = out.entry(bytes).or_insert(0);
        if count as usize > *e { *e = count as usize; }
    }
}

fn build_frequency_map(buf: &[u8]) -> std::collections::HashMap<Vec<u8>, usize> {
    let mut freq = std::collections::HashMap::new();
    let max_len = MAX_STR_LEN.min(buf.len() / 2);
    if max_len < MIN_STR_LEN { return freq; }
    let range = max_len - MIN_STR_LEN;
    let step  = (range / MAX_LENGTHS).max(1);
    let mut len = MIN_STR_LEN;
    while len <= max_len {
        scan_length(buf, len, &mut freq);
        len += step;
    }
    freq
}

fn select_entries(freq: std::collections::HashMap<Vec<u8>, usize>) -> Vec<DictEntry> {
    let mut candidates: Vec<DictEntry> = freq
        .into_iter()
        .filter_map(|(bytes, count)| {
            if count < MIN_FREQ || bytes.len() <= TOKEN_BYTES { return None; }
            let saving = (bytes.len() - TOKEN_BYTES) * count;
            if saving == 0 { return None; }
            Some(DictEntry { bytes, freq: count, saving })
        })
        .collect();
    candidates.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
    let mut selected: Vec<DictEntry> = Vec::new();
    'outer: for c in candidates {
        if selected.len() >= MAX_ENTRIES { break; }
        for e in &selected {
            if e.bytes.windows(c.len()).any(|w| w == c.bytes.as_slice()) {
                continue 'outer;
            }
        }
        selected.push(c);
    }
    selected
}

pub fn build(buf: &[u8]) -> Vec<DictEntry> {
    if buf.is_empty() { return vec![]; }
    select_entries(build_frequency_map(buf))
}

/// Extract non-token regions from a tokenized buffer.
/// Token regions (ESCAPE byte sequences) are skipped.
/// Only residual uncompressed bytes are returned for second-pass analysis.
pub fn extract_residual(tokenized: &[u8]) -> Vec<u8> {
    use crate::tokenizer::codon::ESCAPE;
    let mut residual = Vec::new();
    let mut i = 0;
    while i < tokenized.len() {
        if tokenized[i] == ESCAPE && i + 1 < tokenized.len() {
            i += 2; // skip token
        } else {
            residual.push(tokenized[i]);
            i += 1;
        }
    }
    residual
}

/// Build a second-pass dictionary from residual (non-token) bytes.
/// IDs start at pass1_count + 1 to avoid collision with first dict.
pub fn build_second_pass(residual: &[u8], pass1_count: usize) -> Vec<DictEntry> {
    if residual.is_empty() { return vec![]; }
    let remaining_slots = MAX_ENTRIES.saturating_sub(pass1_count);
    if remaining_slots == 0 { return vec![]; }
    let mut entries = select_entries(build_frequency_map(residual));
    entries.truncate(remaining_slots);
    entries
}

pub fn serialize(entries: &[DictEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for e in entries {
        out.push(e.bytes.len() as u8);
        out.extend_from_slice(&e.bytes);
    }
    out
}

pub fn deserialize(data: &[u8]) -> Result<(Vec<DictEntry>, usize), String> {
    if data.len() < 2 { return Err("dictionary: truncated header".into()); }
    let count = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut pos = 2;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        if pos >= data.len() { return Err(format!("dictionary: truncated at entry {i}")); }
        let len = data[pos] as usize; pos += 1;
        if pos + len > data.len() { return Err(format!("dictionary: entry {i} truncated")); }
        entries.push(DictEntry { bytes: data[pos..pos+len].to_vec(), freq: 0, saving: 0 });
        pos += len;
    }
    Ok((entries, pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rep(s: &str, n: usize) -> Vec<u8> { s.repeat(n).into_bytes() }

    #[test]
    fn test_build_finds_repeated_substring() {
        let buf = rep("hello world ", 20);
        let dict = build(&buf);
        assert!(!dict.is_empty());
        assert!(dict[0].freq >= MIN_FREQ);
    }
    #[test]
    fn test_build_empty() { assert!(build(&[]).is_empty()); }
    #[test]
    fn test_build_no_repeats() {
        let _ = build(&(0u8..100)
            .map(|i| i.wrapping_mul(7).wrapping_add(3)).collect::<Vec<_>>());
    }
    #[test]
    fn test_serialize_deserialize() {
        let buf = rep("the quick brown fox ", 15);
        let dict = build(&buf);
        assert!(!dict.is_empty());
        let ser = serialize(&dict);
        let (restored, consumed) = deserialize(&ser).unwrap();
        assert_eq!(consumed, ser.len());
        for (a, b) in dict.iter().zip(restored.iter()) {
            assert_eq!(a.bytes, b.bytes);
        }
    }
    #[test]
    fn test_max_entries_respected() {
        let mut buf = Vec::new();
        for i in 0..100u8 {
            buf.extend(format!("pattern_{:02x}_data ", i).repeat(5).as_bytes());
        }
        assert!(build(&buf).len() <= MAX_ENTRIES);
    }
    #[test]
    fn test_saving_positive() {
        for e in build(&rep("compress this string repeatedly ", 10)) {
            assert!(e.saving > 0);
        }
    }
    #[test]
    fn test_extract_residual() {
        use crate::tokenizer::codon::ESCAPE;
        // Buffer with some token regions
        let buf = vec![b'h', b'e', ESCAPE, 0x01, b'l', b'o', ESCAPE, 0x02, b'!'];
        let residual = extract_residual(&buf);
        // Should contain only non-token bytes
        assert!(!residual.contains(&ESCAPE));
        assert_eq!(residual, vec![b'h', b'e', b'l', b'o', b'!']);
    }
    #[test]
    fn test_second_pass_finds_residual_patterns() {
        // First pass compresses some patterns, second pass finds more in residual
        let corpus = rep("alpha beta gamma delta ", 30);
        let pass1 = build(&corpus);
        assert!(!pass1.is_empty());
        // Apply first pass
        let tokenized = crate::tokenizer::codon::encode(&corpus, &pass1);
        let residual  = extract_residual(&tokenized);
        // Residual should be smaller than original
        assert!(residual.len() < corpus.len());
        let pass2 = build_second_pass(&residual, pass1.len());
        // May or may not find patterns -- just verify no panic and slot limit
        assert!(pass1.len() + pass2.len() <= MAX_ENTRIES);
    }
    #[test]
    fn test_performance() {
        let msg = "user joined channel general timestamp 1743744000 payload data ";
        let buf: Vec<u8> = msg.repeat(1000).into_bytes();
        let t0 = std::time::Instant::now();
        let dict = build(&buf);
        let ms = t0.elapsed().as_millis();
        println!("74KB dict build: {ms}ms entries={}", dict.len());
        // Only enforce timing in release builds
        #[cfg(not(debug_assertions))]
        assert!(ms < 100, "dict build too slow: {ms}ms");
        #[cfg(debug_assertions)]
        assert!(ms < 5000, "dict build too slow even in debug: {ms}ms");
    }
}
