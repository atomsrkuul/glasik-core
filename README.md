# Glasik Notation (GN)

Open-source domain-adaptive compression for LLM context data.
Learns vocabulary from conversation streams. Beats gzip ratio everywhere. Beats brotli ratio on large chunks.

## Benchmark Results

Measured on ShareGPT V3, Intel i3-1215U, napi-rs addon (no IPC), 3-seed verified.

### GN AC (Aho-Corasick encoder) — production path

| Chunk size | Ratio | vs gzip | vs brotli-6 | p50 | p99 |
|------------|-------|---------|-------------|-----|-----|
| Small (232B) | 1.566x | +16% | -3% | 0.015ms | 0.024ms |
| Medium (989B) | 2.217x | +10% | -3% | 0.047ms | 0.087ms |
| Large (2341B) | **2.897x** | **+15%** | **+4%** | **0.072ms** | **0.136ms** |

### vs baselines (medium chunks, seed=42)

| Method | Ratio | p50 | p99 |
|--------|-------|-----|-----|
| gzip-1 | 1.954x | 0.020ms | 0.028ms |
| gzip-6 | 2.012x | 0.025ms | 0.036ms |
| brotli-1 | 1.880x | 0.014ms | 0.019ms |
| brotli-6 | 2.276x | 0.046ms | 0.072ms |
| **GN AC** | **2.217x** | **0.047ms** | **0.087ms** |
| GN L2 (codon) | 2.212x | 1.608ms | 10.285ms |

## Architecture
Conversation stream
→ GN sliding window (learns domain vocabulary)
→ Aho-Corasick O(n) tokenizer (all patterns simultaneously)
→ libdeflate (entropy coding)
→ compressed output

**Key properties:**
- **O(n) matching**: Aho-Corasick automaton, single pass over input
- **Domain-adaptive**: window learns from stream, rebuilds vocab every 50 chunks
- **No IPC**: napi-rs Rust addon, runs in-process with Node.js
- **Verified**: 3-seed reproducibility across ShareGPT/WildChat/LMSYS/Ubuntu-IRC

## Why GN beats gzip

gzip finds repetition within one buffer (32KB LZ77 window).
GN finds repetition across buffers (domain vocabulary learned from the stream).
They are complementary — GN's cross-chunk patterns + deflate's intra-chunk compression.

## Why GN beats brotli on large chunks

Brotli's static dictionary is trained on web text.
GN's dynamic dictionary adapts to the actual conversation domain.
On LLM conversation data, domain-specific vocabulary outperforms generic web dictionary.

## NLNet NGI Zero Commons Fund

Application #2026-06-023.

## Status

- GN AC: production encoder, O(n) Aho-Corasick, verified
- GN L2 codon: reference encoder, high ratio, 10ms p99
- ANS entropy: implemented, benchmarked, available
- napi-rs addon: Node.js production path
- PyO3: Python research path

## License

MIT
