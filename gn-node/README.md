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

## Benchmark Results (v4.4.4)

Tested on ShareGPT, WildChat, LMSYS corpora (3 seeds each, 2000 chunks/run).

**Compression ratio vs brotli-6 (warm session,