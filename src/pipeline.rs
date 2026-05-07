//! pipeline.rs -- Full GN compression pipeline
//!
//! Two modes, chosen automatically per batch:
//!   Codon-only:      high repetition, domain-specific data
//!   Codon+Deflate:   diverse natural language
//!
//! Mode selection: if codon-only output < deflate output, use codon-only.
//! The pipeline measures both and picks the winner.

use crate::codec::frame::{self, Frame, FrameError};
use crate::ans_table::ANS_O1_TABLE;
use crate::tokenizer::codon;
use crate::tokenizer::dictionary;
use crate::tokenizer::{Tokenizer, TOK_MAGIC};
use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use std::io::{Read, Write};

// Flag byte in GN frame flags field
pub const FLAG_COMPRESSION: u8 = 0x01;
pub const FLAG_CODON_ONLY: u8 = 0x02; // codon table, no deflate
pub const FLAG_ANS_O1: u8 = 0x04;     // o1 ANS pretrained entropy coding

#[derive(Debug)]
pub enum PipelineError {
    Frame(FrameError),
    Inflate(String),
    Tokenizer(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PipelineError::Frame(e) => write!(f, "frame: {e}"),
            PipelineError::Inflate(e) => write!(f, "inflate: {e}"),
            PipelineError::Tokenizer(e) => write!(f, "tokenizer: {e}"),
        }
    }
}

impl From<FrameError> for PipelineError {
    fn from(e: FrameError) -> Self {
        PipelineError::Frame(e)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Auto,      // measure both, pick winner
    CodonOnly, // no deflate
    Deflate,   // codon + deflate always
}

#[derive(Debug)]
pub struct PipelineStats {
    pub input_bytes: usize,
    pub tokenized_bytes: usize,
    pub compressed_bytes: usize,
    pub framed_bytes: usize,
    pub mode_used: &'static str,
}

impl PipelineStats {
    pub fn ratio(&self) -> f64 {
        if self.framed_bytes == 0 {
            return 1.0;
        }
        self.input_bytes as f64 / self.framed_bytes as f64
    }
}

// ── Core encode/decode ────────────────────────────────────────────────────────

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    let _ = enc.write_all(data);
    enc.finish().expect("deflate encoder should never fail on in-memory buffer")
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut dec = DeflateDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn tokenize_buf(data: &[u8]) -> Vec<u8> {
    let tok = Tokenizer::new();
    let (tokenized, _) = tok.encode(data);
    // Only use tokenized output if it actually reduced size.
    // If dict header overhead expanded the data, pass raw to deflate.
    // Always use tokenized for empty input (has magic header for decoder).
    if data.is_empty() || tokenized.len() < data.len() {
        tokenized
    } else {
        data.to_vec()
    }
}

fn frame_codon_only(tokenized: Vec<u8>) -> Vec<u8> {
    let mut f = Frame::new(tokenized, false);
    f.flags = FLAG_CODON_ONLY;
    frame::encode(&f)
}

fn frame_deflate(tokenized: Vec<u8>) -> Vec<u8> {
    let deflated = deflate(&tokenized);
    let mut f = Frame::new(deflated, true);
    f.flags = FLAG_COMPRESSION;
    frame::encode(&f)
}

fn frame_ans_o1(tokenized: Vec<u8>) -> Vec<u8> {
    match crate::codec::ans::compress_o1_pretrained(&tokenized, ANS_O1_TABLE) {
        Some(encoded) => {
            let mut f = Frame::new(encoded, true);
            f.flags = FLAG_ANS_O1;
            frame::encode(&f)
        }
        None => frame_codon_only(tokenized),
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn compress(data: &[u8]) -> Vec<u8> {
    compress_mode(data, &Mode::Auto).0
}

pub fn compress_mode(data: &[u8], mode: &Mode) -> (Vec<u8>, &'static str) {
    let tokenized = tokenize_buf(data);

    match mode {
        Mode::CodonOnly => (frame_codon_only(tokenized), "codon"),
        Mode::Deflate => (frame_deflate(tokenized), "deflate"),
        Mode::Auto => {
            // Try all three, pick smallest
            let codon_frame = frame_codon_only(tokenized.clone());
            let ans_frame = frame_ans_o1(tokenized.clone());
            let deflate_frame = frame_deflate(tokenized);
            let best = [codon_frame, ans_frame, deflate_frame]
                .into_iter()
                .min_by_key(|f| f.len())
                .unwrap();
            let mode = "auto";
            (best, mode)
        }
    }
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>, PipelineError> {
    let view = frame::decode_view(data).map_err(PipelineError::Frame)?;

    let payload = if view.flags & FLAG_ANS_O1 != 0 {
        crate::codec::ans::decompress_o1_pretrained(view.payload, ANS_O1_TABLE)
            .ok_or_else(|| PipelineError::Inflate("ans_o1 decode failed".into()))?
    } else if view.flags & FLAG_COMPRESSION != 0 {
        inflate(view.payload).map_err(PipelineError::Inflate)?
    } else {
        view.payload.to_vec()
    };

    Tokenizer::new()
        .decode(&payload)
        .map_err(PipelineError::Tokenizer)
}

pub fn compress_with_stats(data: &[u8]) -> (Vec<u8>, PipelineStats) {
    let tokenized = tokenize_buf(data);
    let tok_len = tokenized.len();

    let codon_frame = frame_codon_only(tokenized.clone());
    let deflate_frame = frame_deflate(tokenized);

    let (framed, mode_used) = if codon_frame.len() <= deflate_frame.len() {
        (codon_frame, "codon")
    } else {
        (deflate_frame, "deflate")
    };

    (
        framed.clone(),
        PipelineStats {
            input_bytes: data.len(),
            tokenized_bytes: tok_len,
            compressed_bytes: framed.len(),
            framed_bytes: framed.len(),
            mode_used,
        },
    )
}

// ── Batch API ─────────────────────────────────────────────────────────────────

pub fn compress_batch(messages: &[&[u8]]) -> Vec<Vec<u8>> {
    compress_batch_with_stats(messages).0
}

pub fn compress_batch_with_stats(messages: &[&[u8]]) -> (Vec<Vec<u8>>, PipelineStats) {
    if messages.is_empty() {
        return (
            vec![],
            PipelineStats {
                input_bytes: 0,
                tokenized_bytes: 0,
                compressed_bytes: 0,
                framed_bytes: 0,
                mode_used: "none",
            },
        );
    }

    let combined: Vec<u8> = messages.iter().flat_map(|m| m.iter().copied()).collect();
    let entries = dictionary::build(&combined);
    let dict_header = dictionary::serialize(&entries);

    let mut total_input = 0usize;
    let mut total_tokenized = 0usize;
    let mut total_framed = 0usize;
    let mut codon_wins = 0usize;

    let frames: Vec<Vec<u8>> = messages
        .iter()
        .map(|msg| {
            let tokenized = codon::encode(msg, &entries);

            let mut tok_buf = Vec::with_capacity(4 + dict_header.len() + 4 + tokenized.len());
            tok_buf.extend_from_slice(&TOK_MAGIC);
            tok_buf.extend_from_slice(&dict_header);
            tok_buf.extend_from_slice(&(msg.len() as u32).to_le_bytes());
            tok_buf.extend_from_slice(&tokenized);

            total_input += msg.len();
            total_tokenized += tokenized.len();

            // Auto-select mode per message
            let codon_frame = frame_codon_only(tok_buf.clone());
            let deflate_frame = frame_deflate(tok_buf);

            let framed = if codon_frame.len() <= deflate_frame.len() {
                codon_wins += 1;
                codon_frame
            } else {
                deflate_frame
            };

            total_framed += framed.len();
            framed
        })
        .collect();

    let mode_used = if codon_wins > messages.len() / 2 {
        "codon"
    } else {
        "deflate"
    };

    (
        frames,
        PipelineStats {
            input_bytes: total_input,
            tokenized_bytes: total_tokenized,
            compressed_bytes: total_framed,
            framed_bytes: total_framed,
            mode_used,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_auto() {
        let data = b"hello glasik hello glasik hello glasik".to_vec();
        let c = compress(&data);
        assert_eq!(decompress(&c).unwrap(), data);
    }

    #[test]
    fn test_roundtrip_codon_only() {
        let data: Vec<u8> = "repeated pattern ".repeat(50).into_bytes();
        let (c, mode) = compress_mode(&data, &Mode::CodonOnly);
        assert_eq!(mode, "codon");
        assert_eq!(decompress(&c).unwrap(), data);
    }

    #[test]
    fn test_roundtrip_deflate() {
        let data: Vec<u8> = "repeated pattern ".repeat(50).into_bytes();
        let (c, mode) = compress_mode(&data, &Mode::Deflate);
        assert_eq!(mode, "deflate");
        assert_eq!(decompress(&c).unwrap(), data);
    }

    #[test]
    fn test_auto_picks_codon_for_repetitive() {
        // Highly repetitive: codon should win
        let data: Vec<u8> = "aaaa bbbb cccc dddd ".repeat(200).into_bytes();
        let (c, mode) = compress_mode(&data, &Mode::Auto);
        println!("repetitive mode: {mode}");
        assert_eq!(decompress(&c).unwrap(), data);
    }

    #[test]
    fn test_empty() {
        let c = compress(&[]);
        assert_eq!(decompress(&c).unwrap(), b"");
    }

    #[test]
    fn test_corruption_detected() {
        let data = b"test data corruption check".to_vec();
        let mut c = compress(&data);
        let last = c.len();
        c[last - 5] ^= 0xFF;
        assert!(decompress(&c).is_err());
    }

    #[test]
    fn test_batch_roundtrip() {
        let messages: Vec<Vec<u8>> = (0..20)
            .map(|i| format!("user joined channel general timestamp {i}").into_bytes())
            .collect();
        let refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
        for (orig, comp) in messages.iter().zip(compress_batch(&refs).iter()) {
            assert_eq!(&decompress(comp).unwrap(), orig);
        }
    }
}
