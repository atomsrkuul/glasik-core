# Glasik Notation Core

[![npm version](https://img.shields.io/npm/v/gni-compression.svg)](https://www.npmjs.com/package/gni-compression)
[![npm downloads](https://img.shields.io/npm/dm/gni-compression.svg)](https://www.npmjs.com/package/gni-compression)
[![Known Vulnerabilities](https://snyk.io/test/npm/gni-compression/badge.svg)](https://security.snyk.io/package/npm/gni-compression)


Glasik Notation Core is a Rust compression engine for LLM conversation data, agent memory, chat logs, and other repeated message-style text.

The main idea is simple: normal compressors treat text as bytes, but conversation data has structure. Chats repeat roles, message shapes, JSON fields, tool-call patterns, prompt fragments, and common phrases. GN tries to learn that structure first, split it into cleaner streams, and then let normal compression work on something easier.

This project is an experiment in domain-adaptive lossless compression. It is not trying to replace gzip, brotli, or zstd directly. It is exploring what happens when a compressor understands the shape of the data before entropy coding begins.

## Install

```bash
npm install gni-compression
```

Linux x64 only for now.

```js
const { compress, decompress } = require('gni-compression');

const compressed = await compress(Buffer.from(messageString));
const restored = await decompress(compressed);
// restored.toString() === messageString
```

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

Measured on 5 conversation corpora: ShareGPT, WildChat, LMSYS, Ubuntu IRC, and Claude conversations. Each corpus tested across 3 independent seeds (1337, 2718, 31415), 1000 turns per run. Round-trip lossless verification passed 50/50 on all corpora.

GN split-stream with GCdict (HEADLINE):

| Corpus | GN+GCdict | brotli-6 | vs brotli-6 |
|---|---:|---:|---:|
| Claude conversations | 2.72x | 2.26x | +20.5% |
| ShareGPT | 2.72x | 2.35x | +15.5% |
| LMSYS | 2.55x | 2.24x | +14.1% |
| WildChat | 2.35x | 2.18x | +7.5% |
| Ubuntu IRC | 1.99x | 1.21x | +64% |

Ratios are averages across 3 seeds. Claude conversations show the strongest gains because Claude output has consistent structure and vocabulary that GN's domain dictionary learns effectively.

GN split-stream base (no GCdict):

| Corpus | GN ratio | vs brotli-6 |
|---|---:|---:|
| ShareGPT | 2.44x | +3.8% |
| LMSYS | 2.32x | +3.8% |
| Claude conversations | 2.41x | +6.8% |
| WildChat | 2.12x | -2.6% |
| Ubuntu IRC | 1.74x | +43% |

Latency per batch of 8 messages: GN p50 0.30-0.44ms vs brotli p50 0.32-0.48ms. GN is consistently faster at p99 across all corpora.

These numbers show that domain-aware preprocessing with a trained vocabulary can outperform mature general-purpose compressors on conversation-shaped data. That is the interesting part.

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
