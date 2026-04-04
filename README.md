# glasik-core

Rust implementation of the Glasik Notation (GN) compression architecture.

JS reference implementation: https://github.com/atomsrkuul/glasik-notation

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
