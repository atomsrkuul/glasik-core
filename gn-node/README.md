# gni-compression

Domain-adaptive lossless compression for LLM conversation data.

## Benchmark Results (v4.3.0)

GN V4 beats brotli-6 across all LLM corpora with full lossless round-trip.

### Batch compression (batch=8, recommended)
| Corpus | vs brotli-6 | Round-trip |
|--------|-------------|------------|
| WildChat | +13.31% | 300/300 |
| ShareGPT | ~+20% | 300/300 |
| LMSYS | ~+19% | 300/300 |
| Ubuntu IRC | ~+50% | 300/300 |
| Claude convos | ~+27% | 300/300 |

### Single-message (gnSplitRawV4)
| Corpus | vs brotli-6 | Round-trip |
|--------|-------------|------------|
| WildChat | -4.19% | 300/300 |
| ShareGPT | +1.97% | 300/300 |
| LMSYS | +0.99% | 300/300 |
| Ubuntu IRC | +45.01% | 300/300 |
| Claude convos | +20.69% | 300/300 |

## API

```js
const {native} = require('gni-compression');

// V4 encode — returns [toks, lits, runs]
const [toks, lits, runs] = await native.gnSplitRawV4(buffer);

// V4 decode — lossless round-trip
const restored = await native.gnMergeRawV4(toks, lits, runs);

// V1 encode — highest ratio, no round-trip
const [tok, lit] = await native.gnSplitRaw([buffer]);
```

## Install
```
npm install gni-compression
```
Linux x64 only.
