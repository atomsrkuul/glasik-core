# Glasik Notation Core

Glasik Notation Core is a Rust compression engine for LLM conversation data, agent memory, chat logs, and other repeated message-style text.

The main idea is simple: normal compressors treat text as bytes, but conversation data has structure. Chats repeat roles, message shapes, JSON fields, tool-call patterns, prompt fragments, and common phrases. GN tries to learn that structure first, split it into cleaner streams, and then let normal compression work on something easier.

This project is an experiment in domain-adaptive lossless compression. It is not trying to replace gzip, brotli, or zstd directly. It is exploring what happens when a compressor understands the shape of the data before entropy coding begins.

## Why this exists

LLM context is expensive to store, send, search, and replay. Agent systems also create a lot of repeated text: prompts, instructions, logs, tool calls, memory shards, and conversation history.

General-purpose compressors are already very strong, but they do not explicitly know what a chat turn is. They do not know the difference between a role marker, a repeated tool schema, a natural language sentence, or a memory fragment.

GN explores a middle layer:

1. Learn repeated patterns from a conversation stream
2. Tokenize those patterns with a fast automaton
3. Split token data and literal text into separate streams
4. Compress those streams independently
5. Reconstruct the original data losslessly

The long-term goal is a practical compression layer for agent memory, LLM context storage, chat archives, and structured text streams.

## Current status

This is active research and engineering work.

The core Rust engine is working. The project includes benchmark scripts, Python bindings, Node/NAPI work, and experimental compression modes. Some modes are production-oriented, while others are research paths used to test new ideas.

The most important thing to know:

GN is lossless when using the verified reversible paths.

The split-stream benchmark path shows strong compression results, but the project is still evolving toward a clean packaged API that keeps those ratios while also preserving easy decompression.

## Benchmark results

Measured on message-style corpora including ShareGPT, WildChat, LMSYS, and Ubuntu IRC.

Current split-stream benchmark results:

| Corpus | GN ratio | vs gzip | vs brotli-6 |
|---|---:|---:|---:|
| ShareGPT | 2.49x to 2.52x | about +15% | about +2% |
| WildChat | 2.48x to 2.51x | about +15% | about +2% |
| LMSYS | 2.50x to 2.56x | about +14% | about +2% |
| Ubuntu IRC | 2.50x to 2.54x | about +14% to +15% | about +1% to +2% |

Baseline comparison:

| Method | Ratio | Notes |
|---|---:|---|
| gzip-1 | 2.097x | Fast gzip baseline |
| gzip-6 | 2.181x | Standard gzip baseline |
| brotli-1 | 2.023x | Fast brotli baseline |
| brotli-6 | 2.472x | Strong brotli baseline |
| GN split batch=8 | 2.49x to 2.56x | Main split-stream benchmark path |
| GN AC single | 2.13x to 2.20x | Single-message reversible path |
| GN L2 codon | 2.36x to 2.49x | Higher-ratio experimental path |

These numbers are not meant to claim that GN is universally better than existing compressors. They show that domain-aware preprocessing can compete with mature general-purpose compressors on conversation-shaped data.

That is the interesting part.

## How it works

The current architecture looks like this:

```text
conversation stream
  -> sliding window vocabulary learner
  -> Aho-Corasick tokenizer
  -> token stream + literal stream
  -> independent compression
  -> compressed frame

Why split streams?

Mixed text contains both natural language and repeated structure. If token markers are mixed directly into the byte stream, they can pollute the patterns that gzip or brotli would normally find. GN separates the symbolic token stream from the literal text stream so each stream becomes easier to compress.

The token stream is small and repetitive.

The literal stream stays closer to clean text.

The compressor gets cleaner data instead of one noisy mixed stream.

Key features

Lossless compression on verified reversible paths

Rust core for speed and low-level control

Aho-Corasick matching for fast pattern detection

Sliding-window vocabulary learning

Split-stream compression experiments

Python bindings through PyO3

Node/NAPI package work in progress

Benchmarks against gzip and brotli

Focused on LLM conversations, agent memory, logs, and structured text

Repository layout
bench/        benchmark scripts and corpus tests
gn-api/       API experiments
gn-node/      Node/NAPI package work
python/       Python bindings and experiments
scripts/      helper scripts
src/          Rust compression core
Build
git clone https://github.com/atomsrkuul/glasik-core
cd glasik-core
cargo build --release

Run tests:

cargo test

Run benchmarks:

ls bench

Some benchmark scripts expect local corpus files. The benchmark setup is still being cleaned up so results can be reproduced more easily from a fresh clone.

Project direction

The next major focus is GNCompressorV2.

The goal is to make the high-ratio split-stream benchmark path available as a clean package API while keeping decompression practical and lossless.

The current research direction is a reversible V2 stream format with separate lanes:

token stream
position stream
literal stream

This should keep the compression benefits of the raw split-stream model while adding the missing placement information needed for standalone decompression.

In plain terms:

The benchmark path compresses extremely well because it stores very little extra structure.

The reversible path needs enough structure to rebuild the original message.

GNCompressorV2 is about finding the best version of both.

Why this matters

LLM systems are creating more text than people realize. Every agent loop, chat session, memory store, tool call, and prompt chain produces repeated structure. Most of that structure is invisible to normal compression algorithms.

GN asks a simple question:

What if compression for AI systems started by understanding the shape of AI-generated text?

That does not mean replacing traditional compressors. It means giving them cleaner input.

If that works, compression becomes more than storage savings. It becomes part of how agents remember, transmit, and organize their own context.

Current limitations

This project is early.

The benchmark path and the packaged reversible path are not fully unified yet.

Some scripts assume local datasets.

Some APIs are experimental.

Documentation is still being improved.

The project should be treated as an active research prototype, not a finished compression library.

Good first areas for contribution

Improve benchmark reproducibility

Add corpus download helpers

Add zstd comparisons

Document the binary frame format

Help design the reversible V2 split-stream format

Improve Node package ergonomics

Add more round-trip tests

Write examples for compressing chat logs

License

MIT

## Author note

GN started as an experiment in semantic compression for message and memory streams. The surprising part is not that it beats every compressor everywhere. It does not.

The surprising part is that a domain-aware preprocessing layer can get close to mature compressors, and sometimes compete with them, by separating structure before compression starts.

That feels worth exploring.
