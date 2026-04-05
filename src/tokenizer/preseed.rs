//! preseed.rs -- Pre-seeded category dictionaries
//!
//! Ported from gn-context-dictionary.js (Glasik/Robert, 2024)
//!
//! Three categories detected by heuristic:
//!   Json -- starts with { or [, structural patterns
//!   Text -- natural language, common English words
//!   Log  -- error/warn/info log patterns
//!
//! Pre-seeded entries are merged with the adaptive rolling hash
//! dictionary. Known patterns start with high artificial frequency
//! so they survive the selection step.

use crate::tokenizer::dictionary::DictEntry;

#[derive(Debug, Clone, PartialEq)]
pub enum Category {
    Json,
    Text,
    Log,
    Mixed,
}

/// Detect corpus category from first 256 bytes.
/// O(1) heuristic -- no full scan needed.
pub fn detect(buf: &[u8]) -> Category {
    let sample = &buf[..buf.len().min(256)];
    let trimmed = sample.iter().position(|&b| b > 0x20)
        .map(|i| &sample[i..])
        .unwrap_or(sample);

    // Log first -- [ERROR] starts with [ which would fool JSON check
    let log_markers: &[&[u8]] = &[
        b"[ERROR]", b"[WARN]", b"[INFO]", b"[DEBUG]", b"Exception:", b"Error:",
        b"FATAL", b"stack trace", b"Traceback",
    ];
    for marker in log_markers {
        if sample.windows(marker.len()).any(|w| w == *marker) {
            return Category::Log;
        }
    }

    // JSON: starts with { or [
    if trimmed.first() == Some(&b'{') || trimmed.first() == Some(&b'[') {
        return Category::Json;
    }

    // Mixed: contains both prose and structured data
    let has_json = sample.windows(2).any(|w| w == b":{" || w == b"\":");
    let has_prose = sample.windows(5).any(|w| w == b" the " || w == b" and ");
    if has_json && has_prose {
        return Category::Mixed;
    }

    Category::Text
}

/// Return pre-seeded dictionary entries for a category.
/// Artificial high frequency ensures these survive selection.
pub fn preseed(category: &Category) -> Vec<DictEntry> {
    let patterns: &[&[u8]] = match category {
        Category::Json => &[
            b"{\"",        // {"
            b"\":\"",    // ":"
            b"\",\"",   // ","
            b":null",
            b":true",
            b":false",
            b"\"status\":",
            b"\"error\":",
            b"\"data\":",
            b"\"message\":",
            b"\"timestamp\":",
            b"\"id\":",
            b"\"type\":",
            b"\"name\":",
        ],
        Category::Text => &[
            b" the ",
            b" and ",
            b" of ",
            b" to ",
            b" in ",
            b" is ",
            b" for ",
            b" it ",
            b" be ",
            b" that ",
            b" with ",
            b" this ",
            b" are ",
            b" as ",
            b" was ",
            b" have ",
            b" not ",
            b" from ",
            b" or ",
            b" an ",
            b" can ",
            b" you ",
            b" we ",
            b" they ",
            b" which ",
            b" will ",
            b" on ",
            b" at ",
            b" by ",
            b" but ",
        ],
        Category::Log => &[
            b"[ERROR]",
            b"[WARN]",
            b"[INFO]",
            b"[DEBUG]",
            b"Exception:",
            b"Error:",
            b"Warning:",
            b"at ",
            b"caused by",
            b"stack trace",
        ],
        Category::Mixed => &[
            b" the ",
            b" and ",
            b":null",
            b":true",
            b":false",
            b"\":\"",
        ],
    };

    patterns.iter().filter_map(|p| {
        if p.len() <= 2 { return None; } // too short to save
        let saving = (p.len() - 2) * 50; // artificial high freq
        Some(DictEntry {
            bytes:  p.to_vec(),
            freq:   50,  // artificial -- survives selection
            saving,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_json() {
        assert_eq!(detect(b"{\"key\": \"value\"}"), Category::Json);
        assert_eq!(detect(b"[1, 2, 3]"), Category::Json);
    }

    #[test]
    fn test_detect_log() {
        assert_eq!(detect(b"[ERROR] something went wrong"), Category::Log);
        assert_eq!(detect(b"Exception: null pointer"), Category::Log);
    }

    #[test]
    fn test_detect_text() {
        assert_eq!(detect(b"the quick brown fox jumps over the lazy dog"), Category::Text);
    }

    #[test]
    fn test_preseed_not_empty() {
        assert!(!preseed(&Category::Json).is_empty());
        assert!(!preseed(&Category::Text).is_empty());
        assert!(!preseed(&Category::Log).is_empty());
    }

    #[test]
    fn test_preseed_entries_valid() {
        for cat in [Category::Json, Category::Text, Category::Log, Category::Mixed] {
            for entry in preseed(&cat) {
                assert!(entry.bytes.len() > 2, "entry too short: {:?}", entry.bytes);
                assert!(entry.saving > 0);
            }
        }
    }

    #[test]
    fn test_roundtrip_with_preseed() {
        use crate::tokenizer::codon::{encode, decode};
        let entries = preseed(&Category::Text);
        let buf = b"the quick brown fox and the lazy dog and the cat".to_vec();
        let enc = encode(&buf, &entries);
        let dec = decode(&enc, &entries);
        assert_eq!(dec, buf, "roundtrip failed");
        assert!(enc.len() < buf.len(), "should compress: {} -> {}", buf.len(), enc.len());
    }
}
