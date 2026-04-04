//! pipeline.rs -- Full GN compression pipeline
//!
//! Connects tokenizer -> frame codec into a single interface.
//!
//! compress(data: &[u8]) -> Vec<u8>
//!   tokenize -> deflate -> frame
//!
//! decompress(data: &[u8]) -> Result<Vec<u8>, PipelineError>
//!   decode frame -> inflate -> detokenize
//!
//! This is the layer PyO3 bindings expose to Python.
//! Message serialization (varint field encoding) is handled
//! by the caller -- pipeline operates on raw bytes.

use flate2::{
    write::DeflateEncoder,
    read::DeflateDecoder,
    Compression,
};
use std::io::{Read, Write};

use crate::{
    codec::frame::{self, Frame, FrameError},
    tokenizer::Tokenizer,
};

#[derive(Debug)]
pub enum PipelineError {
    Frame(FrameError),
    Inflate(String),
    Tokenizer(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PipelineError::Frame(e)     => write!(f, "frame: {e}"),
            PipelineError::Inflate(e)   => write!(f, "inflate: {e}"),
            PipelineError::Tokenizer(e) => write!(f, "tokenizer: {e}"),
        }
    }
}

impl From<FrameError> for PipelineError {
    fn from(e: FrameError) -> Self { PipelineError::Frame(e) }
}

/// Compress raw bytes through the full GN pipeline.
/// tokenize -> deflate -> frame
pub fn compress(data: &[u8]) -> Vec<u8> {
    let tok = Tokenizer::new();
    let (tokenized, _stats) = tok.encode(data);

    // Deflate the tokenized output
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tokenized).expect("deflate write failed");
    let deflated = encoder.finish().expect("deflate finish failed");

    // Wrap in GN frame
    let f = Frame::new(deflated, true);
    frame::encode(&f)
}

/// Decompress a GN frame back to raw bytes.
/// decode frame -> inflate -> detokenize
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, PipelineError> {
    // Decode frame (zero-copy view)
    let view = frame::decode_view(data).map_err(PipelineError::Frame)?;

    // Inflate
    let payload = if view.is_compressed() {
        let mut decoder = DeflateDecoder::new(view.payload);
        let mut inflated = Vec::new();
        decoder.read_to_end(&mut inflated)
            .map_err(|e| PipelineError::Inflate(e.to_string()))?;
        inflated
    } else {
        view.payload.to_vec()
    };

    // Detokenize
    let tok = Tokenizer::new();
    tok.decode(&payload).map_err(PipelineError::Tokenizer)
}

/// Pipeline statistics for benchmarking.
#[derive(Debug)]
pub struct PipelineStats {
    pub input_bytes:      usize,
    pub tokenized_bytes:  usize,
    pub compressed_bytes: usize,
    pub framed_bytes:     usize,
}

impl PipelineStats {
    pub fn ratio(&self) -> f64 {
        if self.framed_bytes == 0 { return 1.0; }
        self.input_bytes as f64 / self.framed_bytes as f64
    }
}

/// Compress and return stats alongside output.
pub fn compress_with_stats(data: &[u8]) -> (Vec<u8>, PipelineStats) {
    let tok = Tokenizer::new();
    let (tokenized, _tok_stats) = tok.encode(data);

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tokenized).expect("deflate write failed");
    let deflated = encoder.finish().expect("deflate finish failed");

    let f      = Frame::new(deflated.clone(), true);
    let framed = frame::encode(&f);

    let stats = PipelineStats {
        input_bytes:      data.len(),
        tokenized_bytes:  tokenized.len(),
        compressed_bytes: deflated.len(),
        framed_bytes:     framed.len(),
    };
    (framed, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let data = b"hello glasik hello glasik hello glasik".to_vec();
        let compressed   = compress(&data);
        let decompressed = decompress(&compressed).expect("decompress failed");
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compression_ratio() {
        let data: Vec<u8> = "repeated message payload ".repeat(100).into_bytes();
        let (compressed, stats) = compress_with_stats(&data);
        let decompressed = decompress(&compressed).expect("decompress failed");
        assert_eq!(decompressed, data);
        assert!(stats.ratio() > 1.0, "should compress, ratio={:.2}", stats.ratio());
        println!("pipeline: {}B -> {}B ratio={:.2}x", data.len(), compressed.len(), stats.ratio());
    }

    #[test]
    fn test_empty() {
        let compressed   = compress(&[]);
        let decompressed = decompress(&compressed).expect("empty decompress failed");
        assert_eq!(decompressed, b"");
    }

    #[test]
    fn test_corruption_detected() {
        let data = b"test data for corruption check".to_vec();
        let mut compressed = compress(&data);
        let last = compressed.len();
        compressed[last - 5] ^= 0xFF;
        assert!(decompress(&compressed).is_err());
    }
}
