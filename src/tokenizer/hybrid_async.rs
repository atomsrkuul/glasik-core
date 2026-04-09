//! hybrid_async.rs -- Async vocab swap hybrid encoder
//!
//! Hot path: O(n) scan reads atomic Arc<VocabSnapshot> lock-free
//! Learn path: ingest_fast() updates window every chunk
//! Rebuild: atomic swap every 50 (cold) or 100 (warm) chunks

use std::sync::Arc;
use arc_swap::ArcSwap;
use crate::tokenizer::lz77_gn::{GNPrefixTokenizer, PrefixIndex};
use crate::tokenizer::dictionary::DictEntry;
use crate::tokenizer::sliding_v2::SlidingTokenizerV2;
use crate::static_dict;

pub struct VocabSnapshot {
    pub index: PrefixIndex<4>,
    pub generation: u64,
    pub entry_count: usize,
}

pub struct HybridAsyncEncoder {
    vocab: Arc<ArcSwap<VocabSnapshot>>,
    window: SlidingTokenizerV2,
    compressor: libdeflater::Compressor,
    chunks_since_rebuild: u64,
    total_chunks: u64,
    generation: u64,
}

impl HybridAsyncEncoder {
    pub fn new() -> Self {
        let static_entries = static_dict::load_static_dict();
        let dict: Vec<DictEntry> = static_entries.iter().map(|(b,f,s)| DictEntry {
            bytes: b.clone(), freq: *f as usize, saving: *s as usize
        }).collect();
        let index = PrefixIndex::<4>::build(&dict);
        let n = dict.len();
        let snapshot = Arc::new(ArcSwap::new(Arc::new(VocabSnapshot {
            index, generation: 0, entry_count: n
        })));
        let window = SlidingTokenizerV2::new_with_static(static_entries);
        HybridAsyncEncoder {
            vocab: snapshot, window,
            compressor: libdeflater::Compressor::new(libdeflater::CompressionLvl::default()),
            chunks_since_rebuild: 0,
            total_chunks: 0,
            generation: 0,
        }
    }

    fn rebuild_interval(&self) -> u64 {
        if self.total_chunks < 100 { 50 } else { 100 }
    }

    fn rebuild_vocab(&mut self) {
        let (_, raw) = self.window.export_dict();
        let mut dict: Vec<DictEntry> = raw.into_iter()
            .map(|(bytes, freq, saving)| DictEntry {
                bytes, freq: freq as usize, saving: saving as usize
            }).collect();
        dict.sort_unstable_by(|a, b| b.saving.cmp(&a.saving));
        self.generation += 1;
        let n = dict.len();
        let index = PrefixIndex::<4>::build(&dict);
        self.vocab.store(Arc::new(VocabSnapshot {
            index, generation: self.generation, entry_count: n
        }));
        self.chunks_since_rebuild = 0;
    }

    pub fn encode(&mut self, buf: &[u8]) -> Vec<u8> {
        self.total_chunks += 1;
        self.chunks_since_rebuild += 1;

        // Learn every 4th chunk -- amortizes ingest_fast cost
        // ingest_fast = 0.9ms, amortized over 4 = 0.225ms overhead
        if self.total_chunks % 4 == 0 {
            self.window.ingest_fast(buf);
        }

        // Adaptive rebuild
        if self.chunks_since_rebuild >= self.rebuild_interval() {
            self.rebuild_vocab();
        }

        // Encode with atomic vocab (lock-free)
        let snap = self.vocab.load();
        let tokenized = GNPrefixTokenizer::<4>::tokenize_with_index(buf, &snap.index, true);

        // Deflate using stored compressor (no allocation per call)
        let max = self.compressor.deflate_compress_bound(tokenized.len());
        let mut deflated = vec![0u8; max];
        match self.compressor.deflate_compress(&tokenized, &mut deflated) {
            Ok(n) => { deflated.truncate(n); if deflated.len() < tokenized.len() { deflated } else { tokenized } }
            Err(_) => tokenized
        }
    }

    /// Trigger vocab rebuild -- call from background thread or periodically
    pub fn maybe_rebuild(&mut self) {
        if self.chunks_since_rebuild >= self.rebuild_interval() {
            self.rebuild_vocab();
        }
    }

    pub fn stats(&self) -> (usize, u64, u64) {
        let (entries, batches) = self.window.stats();
        (entries, batches, self.generation)
    }
}

impl Default for HybridAsyncEncoder {
    fn default() -> Self { Self::new() }
}
