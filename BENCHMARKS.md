# Glasik Core Benchmarks

Verified results. All lossless. Scripts in `scripts/`.

## Final Results (April 2026)

**GN L3+static beats brotli-6 on all 4 corpora, all 3 seeds.**

### Configuration
- L1: per-call gn_compress, no sliding state
- L2: global GlasikSlidingV2 (W=20,000), corpus-level window
- L3+static: L2 + pressurization (pk=2-3) + bundled static dictionary (5000 entries)
- Ubuntu-IRC uses chunk fusion (target=800B) before compression
- Static dict: trained on 35k chunks (ShareGPT+WildChat+LMSYS+Ubuntu-IRC)

### Repeatability Benchmark (4 corpora x 3 seeds x n=2000, brotli-6)

| Corpus | gzip-6 | brotli-6 | L2 | L3+static | vs brotli | seeds |
|--------|--------|----------|----|-----------|-----------|-------|
| ShareGPT | 2.178x | 2.453x | 2.403x | 2.522x | **BEATS -2.8%** | 3/3 |
| WildChat | 2.025x | 2.234x | 2.145x | 2.255x | **BEATS -0.9%** | 3/3 |
| LMSYS | 2.079x | 2.322x | 2.231x | 2.393x | **BEATS -3.1%** | 3/3 |
| Ubuntu-IRC | 1.643x | 1.829x | 1.726x | 1.838x | **BEATS -0.5%** | 3/3 |

### Per-seed Results

| Corpus | Seed | gzip | brotli | L2 | L2+static | L3+static | vs brotli |
|--------|------|------|--------|----|-----------|-----------|-----------|
| ShareGPT | 42 | 2.174x | 2.450x | 2.394x | 2.457x | 2.511x | BEATS -2.4% |
| ShareGPT | 123 | 2.183x | 2.458x | 2.410x | 2.469x | 2.534x | BEATS -3.0% |
| ShareGPT | 777 | 2.178x | 2.451x | 2.404x | 2.463x | 2.521x | BEATS -2.8% |
| WildChat | 42 | 2.033x | 2.241x | 2.157x | 2.215x | 2.264x | BEATS -1.0% |
| WildChat | 123 | 2.011x | 2.223x | 2.128x | 2.187x | 2.239x | BEATS -0.7% |
| WildChat | 777 | 2.029x | 2.239x | 2.151x | 2.210x | 2.260x | BEATS -0.9% |
| LMSYS | 42 | 2.080x | 2.325x | 2.237x | 2.316x | 2.400x | BEATS -3.1% |
| LMSYS | 123 | 2.069x | 2.310x | 2.214x | 2.299x | 2.376x | BEATS -2.8% |
| LMSYS | 777 | 2.090x | 2.330x | 2.243x | 2.327x | 2.402x | BEATS -3.0% |
| Ubuntu-IRC | 42 | 1.643x | 1.830x | 1.727x | 1.815x | 1.837x | BEATS -0.4% |
| Ubuntu-IRC | 123 | 1.643x | 1.829x | 1.723x | 1.814x | 1.840x | BEATS -0.6% |
| Ubuntu-IRC | 777 | 1.644x | 1.830x | 1.729x | 1.816x | 1.838x | BEATS -0.4% |

### Key Property: Compression Improves With Stream Length

Static compressors (gzip, brotli) maintain flat ratios regardless of stream length.
GN's ratio improves monotonically as the sliding window accumulates domain vocabulary.

ShareGPT example (seed 42):
| n chunks | gzip-6 | brotli-6 | GN L3+static |
|----------|--------|----------|--------------|
| 500 | 2.208x | 2.498x | 2.390x |
| 1000 | 2.199x | 2.480x | 2.511x |
| 2000 | 2.174x | 2.450x | 2.511x |

### Semantic L3 (GN-native embeddings)

| seed | L3 sequential | L3 semantic | delta |
|------|--------------|-------------|-------|
| 42 | 2.513x | 2.516x | +0.1% |
| 123 | 2.516x | 2.513x | -0.1% |
| 777 | 2.505x | 2.515x | +0.4% |

Semantic L3 with GN-native PPMI embeddings is within noise of sequential.
Nomic-embed-text (768-dim) expected to show meaningful improvement.

## ANS Entropy Coder

| Codec | ShareGPT ratio | vs gzip |
|-------|---------------|---------|
| gzip-6 | 2.082x | 1.000x |
| byte-ANS | 1.233x | 0.591x |
| bit-ANS | 1.212x | 0.582x |
| O1-ANS | 0.551x | 0.264x |

ANS without LZ preprocessing cannot beat gzip. Kept as primitive.

## Batch Pipeline (L1)

| Corpus | GN L1 | gzip | vs gzip | lossless |
|--------|-------|------|---------|----------|
| MEMORY.md | 1.851x | 2.075x | 89% | True |
| ShareGPT-1k | 3.752x | 3.945x | 95% | True |
| Ubuntu-IRC-1k | 2.122x | 2.357x | 90% | True |

## Reproducibility

All benchmarks reproducible with scripts in `scripts/`:
- `scripts/final_bench.py` -- definitive 4-corpus repeatability benchmark
- `scripts/bench_static_repeat.py` -- static dict repeatability
- `scripts/bench_repeat.py` -- L1/L2/L3 without static dict
- `scripts/bench_all3.py` -- L1/L2/L3 3-seed comparison
- `scripts/train_static_dict.py` -- train static dictionary from corpora
- `scripts/semantic_l3.py` -- semantic vs sequential L3 comparison

Corpora required: ShareGPT V3, WildChat, LMSYS-Chat-1M, Ubuntu Dialogue Corpus
