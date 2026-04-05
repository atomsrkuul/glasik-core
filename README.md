# glasik-core

Rust implementation of the Glasik Notation (GN) compression architecture.

GN is a domain-aware compression system built for LLM agent context, message
streams, and notation data. glasik-core is the performance layer — a Rust
library with Python bindings via PyO3.

## What it does

- **Codon-table tokenization** — frequency-analyzed dictionary substitution
  before deflate runs, reducing entropy on repetitive domain data
- **Category-aware pre-seeding** — detects JSON, natural language, and log
  data, seeds the dictionary with known high-value patterns before scanning
- **Sliding window accumulation** — maintains domain vocabulary across batches,
  improving compression the longer it runs on a stream
- **Two-pass encoding** — second pass operates only on non-token residual,
  clean boundary between passes
- **Auto mode** — measures tokenized vs raw size, bypasses tokenization when
  it would expand the data

## Verified benchmarks

| Corpus | glasik-core | gzip | vs gzip | Lossless |
|--------|------------|------|---------|----------|
| MEMORY.md (LLM notation) | 1.849x | 2.075x | 89% | 100% |
| ShareGPT-1k (LLM turns) | 3.752x | 3.945x | 95% | 100% |
| Ubuntu-IRC-1k (chat) | 2.122x | 2.357x | 90% | 100% |

GNI matches gzip on general natural language. On domain-specific streams
the sliding window accumulates vocabulary beyond gzip's 32KB window limit —
compression improves monotonically as the window fills.

JS reference implementation: [glasik-notation](https://github.com/atomsrkuul/glasik-notation)

## Structure

    codec/       frame, varint, CRC32
    tokenizer/   codon table, dictionary
    shards/      crystalline state primitives
    bindings/    PyO3 Python interface

## Build

    cargo build
    cargo build --features python
    cargo test
    cargo bench

## Status

- [ ] Frame codec
- [ ] Varint encoding
- [ ] CRC32
- [ ] Codon tokenizer
- [ ] PyO3 bindings
- [ ] Shard primitives
