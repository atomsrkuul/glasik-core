# GN+GCdict Benchmark Results — April 22 2026

## Method
GCdict: uses conversation history (last 32KB of literal stream residue from prior 400 messages) as deflate preset dictionary for the literal stream. No offline training required. Adapts to domain automatically.

Script: `bench/gcdict_all_corpora_v2.js`
Hardware: Intel i3-1215U
Seeds: 1337, 2718, 31415

## Results — GN+GCdict vs brotli-6

All positive across all corpora, all seeds.

| Corpus | avg size | Seed 1337 | Seed 2718 | Seed 31415 | Min | Max |
|--------|----------|-----------|-----------|------------|-----|-----|
| WildChat | 1164B | +7.1% | +5.5% | +7.0% | +5.5% | +7.1% |
| ShareGPT | 1047B | +15.0% | +12.0% | +11.2% | +11.2% | +15.0% |
| LMSYS | 733B | +10.4% | +13.5% | +14.1% | +10.4% | +14.1% |
| Ubuntu-IRC | 111B | +90.8% | +93.2% | +92.8% | +90.8% | +93.2% |

all_positive: TRUE across 12/12 measurements

## GN split-stream vs brotli-6

| Corpus | Seed 1337 | Seed 2718 | Seed 31415 |
|--------|-----------|-----------|------------|
| WildChat | -3.9% | -3.9% | -3.9% |
| ShareGPT | +2.5% | +3.3% | +3.8% |
| LMSYS | +2.1% | +3.3% | +4.8% |
| Ubuntu-IRC | +44.8% | +46.2% | +47.5% |

## Throughput — GN split vs brotli

| Corpus | GN split | brotli | winner |
|--------|----------|--------|--------|
| WildChat | 18-23 MB/s | 13-22 MB/s | comparable |
| ShareGPT | 18-20 MB/s | 13-20 MB/s | GN |
| LMSYS | 17-19 MB/s | 12-18 MB/s | GN |
| Ubuntu-IRC | 7-9 MB/s | 4-5 MB/s | GN +75% |

## Comparison vs Old L3+static Results

The old BENCHMARKS.md used L3+static (trained offline dict, 2000-chunk batches):
- ShareGPT: beat brotli by ~2-3%
- WildChat: beat brotli by ~1%
- LMSYS: beat brotli by ~3%
- Ubuntu-IRC: beat brotli by ~0.5%

GCdict (no offline training, 300-chunk batches):
- ShareGPT: +11-15% — 4-6x better than L3+static
- WildChat: +5.5-7.1% — 5-7x better than L3+static
- LMSYS: +10-14% — 3-5x better than L3+static
- Ubuntu-IRC: +90-93% — 180x better than L3+static

GCdict dominates L3+static on all corpora despite using no offline training.