//! dictionary.rs -- Frequency analysis and dictionary selection
//!
//! Scans a byte buffer, finds frequently repeated substrings,
//! and selects the most space-efficient entries for the codon table.
//!
//! Selection criteria:
//!   - Substring length: MIN_STR_LEN..=MAX_STR_LEN bytes
//!   - Minimum frequency: MIN_FREQ occurrences
//!   - Net saving: (len - TOKEN_BYTES) * freq > 0
//!   - Max entries: MAX_ENTRIES (fits in u8 token id space)
//!
//! Scan uses stride sampling on large buffers to stay O(n).

pub const MIN_STR_LEN:  usize = 4;
pub const MAX_STR_LEN:  usize = 48;
pub const MIN_FREQ:     usize = 3;
pub const MAX_ENTRIES:  usize = 200;
pub const TOKEN_BYTES:  usize = 2;   // escape byte + id byte
pub const SCAN_CAP:     usize = 65536;

/// A selected dictionary entry.
#[derive(Debug, Clone)]
pub struct DictEntry {
    pub bytes:   Vec<u8>,  // the substring this entry represents
    pub freq:    usize,    // observed frequency in scanned buffer
    pub saving:  usize,    // estimated bytes saved if substituted
}

impl DictEntry {
    pub fn len(&self) -> usize { self.bytes.len() }
}

/// Build a frequency map of substrings in `buf`.
/// Uses stride sampling on large buffers to bound scan time.
fn build_frequency_map(buf: &[u8]) -> std::collections::HashMap<Vec<u8>, usize> {
    let mut freq: std::collections::HashMap<Vec<u8>, usize> =
        std::collections::HashMap::new();

    let stride  = if buf.len() > SCAN_CAP { (buf.len() + SCAN_CAP - 1) / SCAN_CAP } else { 1 };
    let max_len = MAX_STR_LEN.min(buf.len() / 2);

    let mut len = MIN_STR_LEN;
    while len <= max_len {
        let mut i = 0;
        while i + len <= buf.len() {
            let b = buf[i];
            // Only start on printable bytes -- skip varint/binary headers
            if b >= 0x20 || b == b'\n' || b == b'\r' {
                let key = buf[i..i + len].to_vec();
                *freq.entry(key).or_insert(0) += 1;
            }
            i += stride;
        }
        len += 2;
    }
    freq
}

/// Select the best dictionary entries from a frequency map.
/// Greedy: pick highest-saving entries, skip substrings of already-selected.
fn select_entries(freq: std::collections::HashMap<Vec<u8>, usize>) -> Vec<DictEntry> {
    // Filter by min frequency and positive net saving
    let mut candidates: Vec<DictEntry> = freq
        .into_iter()
        .filter_map(|(bytes, count)| {
            if count < MIN_FREQ { return None; }
            let len = bytes.len();
            if len <= TOKEN_BYTES { return None; }
            let saving = (len - TOKEN_BYTES) * count;
            if saving == 0 { return None; }
            Some(DictEntry { bytes, freq: count, saving })
        })
        .collect();

    // Sort by saving descending
    candidates.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));

    // Greedy deduplication: skip entries that are substrings of selected
    let mut selected: Vec<DictEntry> = Vec::new();
    'outer: for candidate in candidates {
        if selected.len() >= MAX_ENTRIES { break; }
        for existing in &selected {
            if existing.bytes.windows(candidate.len())
                              .any(|w| w == candidate.bytes.as_slice()) {
                continue 'outer;
            }
        }
        selected.push(candidate);
    }
    selected
}

/// Analyse a buffer and return selected dictionary entries,
/// sorted by descending saving (highest value first).
pub fn build(buf: &[u8]) -> Vec<DictEntry> {
    let freq = build_frequency_map(buf);
    select_entries(freq)
}

/// Serialise dictionary for frame header.
/// Format: [2B count] [entries: 1B len, len bytes each]
pub fn serialize(entries: &[DictEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for entry in entries {
        out.push(entry.bytes.len() as u8);
        out.extend_from_slice(&entry.bytes);
    }
    out
}

/// Deserialise dictionary from frame header bytes.
/// Returns (entries, bytes_consumed).
pub fn deserialize(data: &[u8]) -> Result<(Vec<DictEntry>, usize), String> {
    if data.len() < 2 {
        return Err("dictionary: truncated header".into());
    }
    let count = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut pos = 2;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        if pos >= data.len() {
            return Err(format!("dictionary: truncated at entry {i}"));
        }
        let len = data[pos] as usize; pos += 1;
        if pos + len > data.len() {
            return Err(format!("dictionary: entry {i} bytes truncated"));
        }
        let bytes = data[pos..pos + len].to_vec(); pos += len;
        entries.push(DictEntry { bytes, freq: 0, saving: 0 });
    }
    Ok((entries, pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repeated(s: &str, n: usize) -> Vec<u8> {
        s.repeat(n).into_bytes()
    }

    #[test]
    fn test_build_finds_repeated_substring() {
        let buf = repeated("hello world ", 20);
        let dict = build(&buf);
        assert!(!dict.is_empty(), "should find repeated substrings");
        // Top entry should contain "hello world" or substring
        assert!(dict[0].freq >= MIN_FREQ);
        assert!(dict[0].saving > 0);
    }

    #[test]
    fn test_build_empty() {
        let dict = build(&[]);
        assert!(dict.is_empty());
    }

    #[test]
    fn test_build_no_repeats() {
        // Random-ish bytes -- unlikely to have MIN_FREQ repeats
        let buf: Vec<u8> = (0..100).map(|i| (i * 7 + 3) as u8).collect();
        let dict = build(&buf);
        // May or may not find entries -- just verify no panic
        let _ = dict;
    }

    #[test]
    fn test_serialize_deserialize() {
        let buf = repeated("the quick brown fox ", 15);
        let dict = build(&buf);
        assert!(!dict.is_empty());

        let serialized = serialize(&dict);
        let (restored, consumed) = deserialize(&serialized).expect("deserialize failed");

        assert_eq!(consumed, serialized.len());
        assert_eq!(restored.len(), dict.len());
        for (a, b) in dict.iter().zip(restored.iter()) {
            assert_eq!(a.bytes, b.bytes, "entry bytes mismatch");
        }
    }

    #[test]
    fn test_max_entries_respected() {
        // Generate highly repetitive data with many distinct patterns
        let mut buf = Vec::new();
        for i in 0..100u8 {
            let s = format!("pattern_{:02x}_data ", i);
            buf.extend(s.repeat(5).as_bytes());
        }
        let dict = build(&buf);
        assert!(dict.len() <= MAX_ENTRIES);
    }

    #[test]
    fn test_saving_positive() {
        let buf = repeated("compress this string repeatedly ", 10);
        let dict = build(&buf);
        for entry in &dict {
            assert!(entry.saving > 0);
            assert!(entry.len() > TOKEN_BYTES);
        }
    }
}
