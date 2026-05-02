# gni-compression

Domain-adaptive lossless compression for LLM conversation data.

## Install

```bash
npm install gni-compression
```

Linux x64 only.

## Usage

```js
const { compress, decompress } = require('gni-compression');

// Lossless compress/decompress any Buffer
const compressed = await compress(Buffer.from(messageString));
const restored = await decompress(compressed);
// Buffer.from(restored).toString() === messageString
```

## Benchmark Results (v4.4.0)

Verified locally (4 corpora x 3 seeds = 12 runs, 50/50 round-trip pass each).

| Corpus | GN+GCdict vs brotli-6 |
\--------|-----------------------|
| WildChat | +6.7 to +8.5% |
| ShareGPT | +42 to +55% |
| LMSYS | +13 to +15% |
| Ubuntu IRC | +91 to +93% |

Gn split-stream stage also beats brotli on throughput across all corpora.

## Source

[github.com/atomsrkuul/glasik-core](https://github.com/atomsrkuul/glasik-core) - MIT

## Academic

arXiv preprint pending (cs.IR)

If you have 3+ published cs papers and would like to endorse this work for arXiv submission:
- arXiv Username: RobertRider00
- Endorsement Code: 7HWUBA (cs.IR)
- Contact: atomsrkuul@gmail.com
