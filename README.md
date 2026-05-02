# Glasik Notation (GN)

Open-source domain-adaptive compression for LLM context data.
Learns vocabulary from conversation streams.

**Beats brotli-6 ratio and p99 latency simultaneously.**

## Benchmark Results

Measured on ShareGPT V3 / WildChat / LMSYS / Ubuntu-IRC.
Intel i3-1215U. PyO3 path. 4 corpora × 3 seeds = 12 independent measurements.

### GN split-stream batch=8 — production path

| Corpus | Ratio | vs gzip | vs brotli-6 | p50 | p99 | MB/s |
|--------|-------|---------|-------------|-----|-----|------|
| ShareGPT | 2.49-2.52x | +15% | +2% | 0.043ms | 0.061ms | 26-27 |
| WildChat | 2.48-2.51x | +15% | +2% | 0.042ms | 0.068ms | 26 |
| LMSYS | 2.50-2.56x | +14% | +2% | 0.044ms | 0.079ms | 23-27 |
| Ubuntu-IRC | 2.50-2.54x | +14-15% | +1-2% | 0.043ms | 0.111ms | 23-24 |

### vs baselines

| Method | Ratio | p50 | p99 | MB/s |
|--------|-------|-----|-----|------|
| gzip-1 | 2.097x | 0.024ms | 0.143ms | 40.0 |
| gzip-6 | 2.181x | 0.024ms | 0.220ms | 37.1 |
| brotli-1 | 2.023x | 0.013ms | 0.023ms | 84.8 |
| brotli-6 | 2.472x | 0.044ms | 0.226ms | 22.0 |
| **GN split b=8** | **2.49-2.56x** | **0.040ms** | **0.056-0.123ms** | **23-27** |
| GN AC (single) | 2.13-2.20x | 0.042ms | 0.099ms | 22-26 |

## Architecture
Conversation stream (batches of 8 chunks)
→ GN sliding window (learns domain vocabulary)
→ Aho-Corasick O(n) tokenizer
→ Split: token ID stream + literal byte stream
→ Raw deflate each stream independently
→ Frame: [2B tok_len][tok_deflated][lit_deflated]

**Why split-stream wins:**
- Mixed stream: gzip sees ESCAPE bytes breaking pattern matching
- Token stream alone: pure symbol stream, highly compressible
- Literal stream alone: clean text, no ESCAPE pollution
- Batch=8: cross-chunk patterns in both streams improve further

**Key properties:**
- O(n) matching via Aho-Corasick automaton
- Domain-adaptive: window learns from stream, rebuilds every 50 chunks
- Lossless: round-trip verified, decode requires same vocab snapshot
- No IPC: napi-rs Rust addon, runs in-process

## Compression modes

| Mode | Ratio | Speed | Use case |
|------|-------|-------|----------|
| GN split b=8 | 2.49-2.56x | 0.040ms | Batch/conversation compression |
| GN split b=16 | 2.54-2.62x | 0.037ms | Larger batches |
| GN AC single | 2.13-2.20x | 0.042ms | Per-message, lossless |
| GN L2 codon | 2.36-2.49x | 1.8ms | Max ratio reference |

## NLNet NGI Zero Commons Fund

Application #2026-06-023.

## License

MIT

## arXiv

Endorsed for submission to cs.IR.
arXiv username: RobertRider00
Paper: *GN: Domain-Adaptive Lossless Compression for LLM Conversation Streams*
