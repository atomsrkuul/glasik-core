# gn-native

Native Rust addon for Node.js — domain-adaptive lossless compression for LLM conversation streams.

## What is GN?

GN (Glasik Notation) is a split-stream tokenized compression codec built for LLM traffic.
It learns vocabulary from conversation patterns and separates token IDs from literal bytes
before compressing each stream independently with raw deflate.

The result: better ratio than brotli-6 on LLM data, with 2-4x better tail latency.

## Verified Benchmarks

Standard protocol: warm 500 chunks, test 300, 4 corpora x 3 seeds = 12 measurements.

### GN split-stream b=8 (production)

| Corpus     | Ratio       | vs gzip | vs brotli-6 | p50      | p99      |
|------------|-------------|---------|-------------|----------|----------|
| ShareGPT   | 2.49-2.52x  | +15%    | +2%         | 0.043ms  | 0.061ms  |
| WildChat   | 2.48-2.51x  | +15%    | +2%         | 0.042ms  | 0.073ms  |
| LMSYS      | 2.50-2.56x  | +14%    | +2%         | 0.044ms  | 0.079ms  |
| Ubuntu-IRC | 2.06-2.09x  | +49%    | +28%        | 0.008ms  | 0.013ms  |

Baselines (same data, per-batch fair comparison):

| Algorithm | Ratio  | p50      | p99      |
|-----------|--------|----------|----------|
| gzip-6    | 2.181x | 0.024ms  | 0.220ms  |
| brotli-6  | 2.472x | 0.044ms  | 0.226ms  |

GN split b=8 p99 never exceeds 0.123ms. Brotli-6 p99 reaches 0.226ms.

### Production metrics (OpenClaw live, April 2026)

- Messages processed: 3,570
- Average ratio: 2.404x
- Maximum ratio: 10.878x
- Total bytes saved: 2,440.9 KB

Real LLM agent traffic. Matches lab predictions.

### Cold-start

With L0 snapshot pre-loaded, GN beats brotli from chunk 0 (2.4903x vs 2.4271x at n=0).

## Architecture

### Split-stream insight

Mixed tokenized streams pollute deflate with structural noise (ESCAPE bytes every 2 bytes).
GN separates token IDs and literal bytes into independent streams, each compressed with raw deflate.
Token stream: pure symbols with skewed distribution. Deflate loves it.
Literal stream: clean text with no structural noise.

### Tiered vocabulary (L0-L3)

- **L0**: Universal (pre-trained, 20k entries, static)
- **L1**: Domain (per shard type, learned online)
- **L2**: Session (sliding window per session)
- **L3**: Chunk (ephemeral N-grams, serialized into frame)

### VTC v3 (Virtual Time Crystal identity)

Every compressed shard has a deterministic crystal identity:
VTC-v3-SHA256(shard_type || session_id || canonical_pairs || literal_hash || sequence_fingerprint)

- Same content + same session = same VTC always
- Different content, session, or shard type = different VTC guaranteed
- Collision-resistant by construction, not just by hash probability
- Includes literal residue (negative space) and emission order fingerprint

### Frame format
[1B shard_type][2B pairs_deflated_len LE][2B l3_ser_len LE][l3_ser][deflated_pairs][deflated_literals]

Self-contained. Given the vocabulary snapshot, fully decodable without external state.

## API

```javascript
const gn = require('gn-native');

// Split-stream batch compression (production, b=8)
const results = await gn.gnCompressSplitBatch(chunks); // Buffer[] -> Buffer (concatenated)

// Single chunk
const compressed = await gn.gnCompressSplit(data);
const decompressed = await gn.gnDecompress(compressed);

// Fractal sharding with VTC identity
const vtc = await gn.gnCompressFractalWithVtc(data, 'user_intent', sessionId);
// Returns: "VTC-v3-<64 hex chars>"

// Fractal compress/decompress
const frame = await gn.gnCompressFractal(data, 'user_intent', sessionId);
const original = await gn.gnDecompressFractal(frame, 'user_intent', sessionId);

// Vocabulary
await gn.gnSaveSnapshot(path);
await gn.gnLoadSnapshot(path);
const stats = await gn.gnWindowStats();

// Health check
const ok = await gn.gnTest(); // returns "binding_ok"
```

### Shard types

`user_intent` | `assistant_response` | `system_message` | `code_block` | `tool_call` | `tool_result` | `generic`

## Installation

```bash
npm install gn-native
```

Requires pre-built `.node` addon for `linux-x64-gnu`.
To build from source:

```bash
git clone https://github.com/atomsrkuul/glasik-core
cd glasik-core/gn-node
npm install
npm run build
```

Requires Rust toolchain and napi-rs CLI.

## Paper

GN: Domain-Adaptive Lossless Compression for LLM Conversation Streams
Robert Rider, Independent Researcher
Pending arXiv cs.IR submission.
GitHub: https://github.com/atomsrkuul/glasik-core (MIT)
