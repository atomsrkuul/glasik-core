// sliding_v2_l4.rs -- Level 4 sliding window with fractal dictionary compression
// Extends SlidingTokenizerV2 with compressed window storage.
// The window is stored compressed using GN pipeline (self-referential).
// Decompressed on-demand, re-compressed after RECOMPRESS_INTERVAL batches.

use crate::tokenizer::sliding_v2::SlidingTokenizerV2;
use crate::level4;

pub const RECOMPRESS_INTERVAL: u64 = 50;

pub struct SlidingTokenizerL4 {
    inner: SlidingTokenizerV2,
    compressed_snapshot: Option<Vec<u8>>,
    last_compress_batch: u64,
}

impl SlidingTokenizerL4 {
    pub fn new() -> Self {
        SlidingTokenizerL4 {
            inner: SlidingTokenizerV2::new(),
            compressed_snapshot: None,
            last_compress_batch: 0,
        }
    }

    pub fn new_with_static(entries: Vec<(Vec<u8>, u64, u64)>) -> Self {
        SlidingTokenizerL4 {
            inner: SlidingTokenizerV2::new_with_static(entries),
            compressed_snapshot: None,
            last_compress_batch: 0,
        }
    }

    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        let result = self.inner.encode(buf);
        let (_, batch_count) = self.inner.stats();

        // Re-compress window snapshot periodically
        if batch_count - self.last_compress_batch >= RECOMPRESS_INTERVAL {
            let (_, entries) = self.inner.export_dict();
            if !entries.is_empty() {
                self.compressed_snapshot = Some(level4::compress_window(&entries));
                self.last_compress_batch = batch_count;
            }
        }
        result
    }

    pub fn stats(&self) -> (usize, u64) {
        self.inner.stats()
    }

    pub fn snapshot_size(&self) -> usize {
        self.compressed_snapshot.as_ref().map(|s| s.len()).unwrap_or(0)
    }

    pub fn export_dict(&self) -> (u32, Vec<(Vec<u8>, u64, u64)>) {
        self.inner.export_dict()
    }

    /// Restore window from compressed snapshot
    pub fn restore_from_snapshot(snapshot: &[u8]) -> Self {
        let entries = level4::decompress_window(snapshot);
        let mut tok = SlidingTokenizerL4::new();
        tok.inner.import_dict(1, entries);
        tok.compressed_snapshot = Some(snapshot.to_vec());
        tok
    }

    pub fn get_snapshot(&self) -> Option<&[u8]> {
        self.compressed_snapshot.as_deref()
    }
}

impl Default for SlidingTokenizerL4 {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l4_encode_decode() {
        let mut tok = SlidingTokenizerL4::new();
        let data = b"user: hello assistant: how can I help you today ".repeat(20);
        for _ in 0..100 {
            tok.encode(&data);
        }
        let (entries, batches) = tok.stats();
        println!("L4: {} entries, {} batches, snapshot={}B",
            entries, batches, tok.snapshot_size());
        assert!(entries > 0);
    }

    #[test]
    fn test_snapshot_restore() {
        let mut tok = SlidingTokenizerL4::new();
        let data = b"user: hello assistant: how can I help you today ".repeat(20);
        for _ in 0..100 {
            tok.encode(&data);
        }
        let snapshot = tok.get_snapshot().expect("snapshot should exist").to_vec();
        println!("Snapshot size: {}B", snapshot.len());

        // Restore from snapshot
        let mut tok2 = SlidingTokenizerL4::restore_from_snapshot(&snapshot);
        let (e1, _) = tok.stats();
        let (e2, _) = tok2.stats();
        println!("Original: {} entries, Restored: {} entries", e1, e2);
        assert!(e2 > 0);
    }
}
