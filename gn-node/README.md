# gni-compression

Domain-adaptive lossless compression for LLM conversation data.

## Benchmark Results (v4.3.0)

Tested via live API (glasik.mooo.com) across 5 corpora, 50 messages each. Full lossless round-trip verified.

### Batch compression
| Corpus | Ratio | Savings | vs brotli-6 |
|--------|-------|---------|-------------|
| WildChat | 4.94x | 79.76% | ~2.4x better |
| ShareGPT | 8.65x | 88.44% | ~4.1x better |
| LMSYS | 10.38x | 90.37% | ~4.9x better |
| Ubuntu IRC | 8.40x | 88.09% | ~4.0x better |
| Claude convos | 12.40x | 91.93% | ~5.9x better |

brotli-6 baseline: ~2.1x on WildChat, ~2.1x on ShareGPT/LMSYS.

Batch mode trains a shared vocabulary (GCdict) across messages in the same session — the more messages, the better the ratio.

## API

\`\`\`js
const {native} = require('gni-compression');

// V4 encode — returns [toks, lits, runs]
const [toks, lits, runs] = await native.gnSplitRawV4(buffer);

// V4 decode — lossless round-trip
const restored = await native.gnMergeRawV4(toks, lits, runs);

// V1 encode — single-message mode
const [tok, lit] = await native.gnSplitRaw([buffer]);
\`\`\`

## REST API

Public endpoint: https://glasik.mooo.com

\`\`\`bash
curl -s -X POST https://glasik.mooo.com/compress-batch \\
  -H 'Content-Type: application/json' \\
  -H 'x-api-key: YOUR_KEY' \\
  -d '{"messages": ["base64msg1", "base64msg2"]}'
\`\`\`

## Install
\`\`\`
npm install gni-compression
\`\`\`
Linux x64 only.
