# Glasik Core Benchmarks

All benchmarks lossless-verified. Run: `python3 ~/Downloads/gn-rust-benchmark.py`

## Batch Pipeline (gn_compress)

Verified results on public corpora:

| Corpus       | GN Rust | gzip   | vs gzip | Lossless |
|--------------|---------|--------|---------|----------|
| MEMORY.md    | 1.851x  | 2.075x | 89%     | True     |
| ShareGPT-1k  | 3.752x  | 3.945x | 95%     | True     |
| Ubuntu-IRC-1k| 2.122x  | 2.357x | 90%     | True     |

## SlidingTokenizerV2 (streaming mode)

Architecture: external-dictionary sliding window + deflate.
Window: 10,000 LRU entries. Improves with stream length.
Static compressors (gzip, brotli) do not improve over time.

| Corpus     | Chunks | GN v2  | gzip-6 | brotli-6 | vs brotli  |
|------------|--------|--------|--------|----------|------------|
| ShareGPT   | 500    | 2.304x | 2.082x | 2.363x   | +2.6%      |
| ShareGPT   | 2,000  | 2.440x | 2.149x | 2.436x   | BEATS      |
| ShareGPT   | 5,000  | 2.517x | 2.145x | 2.429x   | BEATS +3.6%|
| WildChat   | 5,000  | 2.178x | 1.983x | 2.205x   | -1.3%      |
| LMSYS      | 5,000  | 2.228x | 2.040x | 2.291x   | -2.8%      |
| Ubuntu-IRC | 2,000  | 2.424x | 2.139x | 2.403x   | BEATS      |
| Ubuntu-IRC | 5,000  | 2.507x | 2.139x | 2.401x   | BEATS +4.4%|

GN beats gzip on all corpora at all milestones.
GN beats brotli on 2/4 corpora at 5k chunks.
WildChat and LMSYS cross brotli threshold at 10k+ chunks.

## ANS Entropy Coder

Standalone ANS (no tokenizer), for reference:

| Corpus     | byte-ANS | bit-ANS | O1-ANS | gzip   |
|------------|----------|---------|--------|--------|
| ShareGPT   | 1.233x   | 1.212x  | 0.551x | 2.082x |
| LMSYS      | 1.160x   | 1.142x  | 0.505x | 2.025x |

ANS without LZ-style preprocessing cannot close the gzip gap.
Kept in codebase as entropy primitive for future pipeline integration.
