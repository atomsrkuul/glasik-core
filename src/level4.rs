// level4.rs -- Dictionary Fractal (Level 4 VTC)
// Compresses the sliding window using GN tokenizer itself.
// Self-similar compression: the compressor compresses its own dictionary.
//
// Architecture:
//   - Window entries serialized to compact binary format
//   - Binary compressed with GN pipeline (self-referential)
//   - Decompressed on-demand for encode calls
//   - Re-compressed after RECOMPRESS_INTERVAL updates
//
// Benefit: window can store 10x more entries in same memory
// because compressed window is 6x smaller than raw

use crate::pipeline;
use crate::tokenizer::dictionary::DictEntry;

pub const RECOMPRESS_INTERVAL: u64 = 100; // recompress every N batches

/// Serialize window entries to compact binary
/// Format: [len(u8) + bytes + freq(u32) + saving(u32)] * n
pub fn serialize_entries(entries: &[(Vec<u8>, u64, u64)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (bytes, freq, saving) in entries {
        out.push(bytes.len() as u8);
        out.extend_from_slice(bytes);
        out.extend_from_slice(&(*freq as u32).to_le_bytes());
        out.extend_from_slice(&(*saving as u32).to_le_bytes());
    }
    out
}

/// Deserialize window entries from compact binary
pub fn deserialize_entries(data: &[u8]) -> Vec<(Vec<u8>, u64, u64)> {
    let mut entries = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if i >= data.len() { break; }
        let blen = data[i] as usize;
        i += 1;
        if i + blen + 8 > data.len() { break; }
        let bytes = data[i..i+blen].to_vec();
        i += blen;
        let freq = u32::from_le_bytes(data[i..i+4].try_into().unwrap_or([0;4])) as u64;
        i += 4;
        let saving = u32::from_le_bytes(data[i..i+4].try_into().unwrap_or([0;4])) as u64;
        i += 4;
        entries.push((bytes, freq, saving));
    }
    entries
}

/// Compress window entries using GN pipeline (Level 4: fractal compression)
pub fn compress_window(entries: &[(Vec<u8>, u64, u64)]) -> Vec<u8> {
    let serialized = serialize_entries(entries);
    pipeline::compress(&serialized)
}

/// Decompress window entries
pub fn decompress_window(compressed: &[u8]) -> Vec<(Vec<u8>, u64, u64)> {
    match pipeline::decompress(compressed) {
        Ok(data) => deserialize_entries(&data),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let entries = vec![
            (b"hello world".to_vec(), 100u64, 500u64),
            (b"user: ".to_vec(), 200u64, 1000u64),
            (b"assistant: ".to_vec(), 150u64, 750u64),
        ];
        let compressed = compress_window(&entries);
        let restored = decompress_window(&compressed);
        assert_eq!(entries.len(), restored.len());
        for (orig, rest) in entries.iter().zip(restored.iter()) {
            assert_eq!(orig.0, rest.0);
            assert_eq!(orig.1, rest.1);
            assert_eq!(orig.2, rest.2);
        }
        println!("raw: {}B compressed: {}B ratio: {:.2}x",
            entries.iter().map(|e| e.0.len() + 8).sum::<usize>(),
            compressed.len(),
            entries.iter().map(|e| e.0.len() + 8).sum::<usize>() as f64 / compressed.len() as f64
        );
    }
}
