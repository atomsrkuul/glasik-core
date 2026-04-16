# GLASIK PROJECT HANDOFF -- Complete Context Document
# April 16, 2026
# For use when switching between Claude instances, CC, GPro, or Glasik
---
## WHO I AM
Robert Rider, Independent Researcher, Mason Michigan USA
GitHub: github.com/atomsrkuul
Email: atomsrkuul@gmail.com
arXiv username: RobertRider00
arXiv endorsement code: 7HWUBA (cs.IR, needs qualified endorser with 3+ cs papers)
NLNet application: #2026-06-023 (deadline April 29 2026)
dev.to: atomsrkuul
---
## THE CRYSTAL
"The crystal grows" is our phrase for meaningful, verifiable progress on GN.
Not vibes. Not benchmarks that haven't been verified. Real numbers, real code,
real commits. When something is crystal it means:
- Verified across 4 corpora x 3 seeds = 12 measurements minimum
- Pushed to GitHub with honest commit messages
- Numbers match between Python PyO3 and Node.js napi paths
- Round-trip lossless verified
The opposite of crystal is stub -- code that claims to work but doesn't.
---
## HARDWARE
Laptop (primary dev machine):
  Hostname: Buffer
  Username: boot
  OS: Kali Linux
  CPU: Intel i3-1215U
  RAM: 32GB
  Storage: 1TB Crucial NVMe SSD
  Python venv: ~/glasik-core/.venv
Desktop (Optiplex 9020):
  OS: Windows (GTX 970 4GB)
  Ollama running at 192.168.0.2:11434
  Model: qwen2.5:3b
  SSH to desktop: available if needed for GPU work
---
## REPOSITORIES
### glasik-core (Rust compression engine)
  Local:  ~/glasik-core
  Remote: github.com/atomsrkuul/glasik-core
  Branch: master
  PAT required for push (HTTPS auth)
  Key source files:
    src/tokenizer/codon.rs          -- AC encoder, encode_ac_interleaved,
                                       decode_ac_interleaved, build_tiered_ac
    src/tokenizer/sliding_v2.rs     -- SlidingTokenizerV2, encode_ac_split,
                                       hard cap eviction (no evict on hot path)
                                       credit_pattern() method added April 14
    src/tokenizer/lz77_gn.rs        -- GNPrefixTokenizer (dead code marked)
                                       lz77_encode_literals() added (not wired)
                                       lz77_decode_literals() added (not wired)
    src/tokenizer/hybrid_async.rs   -- HybridAsyncEncoder
    src/tokenizer/dictionary.rs     -- rolling hash N-gram, build(), MAX_STR_LEN=32
    src/pipeline.rs                 -- libdeflate pipeline, expect() not unwrap
    src/level4.rs                   -- fractal self-compression of dictionary
    src/sliding_v2_l4.rs            -- L4 wrapper with periodic re-compression
    src/fractal.rs                  -- FractalCompressor (tiered L0-L3, PRODUCTION)
    src/bindings/mod.rs             -- PyO3 Python bindings
    src/lib.rs                      -- module registry
    gn-node/src/lib.rs              -- napi-rs Node.js bindings (29 exports)
    gn-node/gn-native.linux-x64-gnu.node  -- built napi addon
  Build commands:
    Rust:   cd ~/glasik-core && cargo build --release
    Tests:  cd ~/glasik-core && cargo test --release
    PyO3:   cd ~/glasik-core && maturin develop --release --features python
    napi:   cd ~/glasik-core/gn-node && napi build --platform --release
  Key constants:
    MAX_STR_LEN = 32 (dictionary.rs)
    MAX_ENTRIES = 200 (dictionary.rs)
    MAX_WINDOW_ENTRIES = 20000 (sliding_v2.rs) -- hard cap, no eviction
    Token IDs: strict 1-254, no wrapping, 0=reserved
    ESCAPE byte = 0x01
    AC rebuild: every 50 chunks cold, 100 warm (ac_dirty flag)
  LATENCY NOTE: do NOT call ingest_fast/encode per-chunk inside CompressSplitBatch
    -- this triggers FirstByteIndex rebuilds causing 35x latency regression
    -- encode all chunks with current AC, then update window once after batch
    PROMOTE_L3_THRESHOLD = 2 (fractal.rs)
    PROMOTE_L2_THRESHOLD = 8 (fractal.rs)
### Glasik-Workspace (OpenClaw JS agent)
  Local:  ~/.openclaw/workspace
  Remote: github.com/atomsrkuul/Glasik-Workspace
  Branch: master
  Key source files:
    src/gn-split-stream-encoder.js  -- PRODUCTION encoder (real napi bindings)
    src/gn-shards.js                -- unified shard manager
    src/gn-unified.js               -- single entry point
    src/gn-ribosome.js              -- polyribosome shard processing
    src/gn-phase-transition.js      -- phase transition detection
    src/gn-polyribosomes.js         -- parallel shard processing
    src/gn-production-hook.js       -- OpenClaw message compression hook
    src/gn-simple-collector.js      -- metrics collection (no deps)
    src/gn-lattice-bridge.js        -- REST API for metrics
  napi addon path:
    /home/boot/glasik-core/gn-node/gn-native.linux-x64-gnu.node
---
## NAPI EXPORTS (gn-node/src/lib.rs) -- 29 functions
Production compression:
  gnCompressSplitBatch(chunks)        -- PRODUCTION batch, 2.15-2.55x
  gnCompressSplit(data)               -- single chunk split-stream
  gnCompressAc(data)                  -- AC only
  gnCompressL2(data)                  -- sliding window
  gnCompress / gnCompressFast / gnCompressFastSync
  gnCompressHybrid / gnCompressHybridSync
  gnCompressBatch / gnCompressLocal / gnCompressTl
  gnCompressPressurized(target, warm, pk)
  gnDecompress / gnDecompressAc
  gnSplitRaw(chunks)                  -- returns [tok_buf, lit_buf] raw streams
                                         INPUT MUST BE ARRAY e.g. [buf] not buf
                                         used for GCdict pipeline
  gnCompressSplitBatchSync            -- sync variant
Fractal (PRODUCTION):
  gnCompressFractal(data, shard_type, session_id)
  gnDecompressFractal(data, shard_type, session_id)
  gnCompressFractalWithVtc(data, shard_type, session_id) -- returns VTC string
  gnGetPairs(data, shard_type, session_id)
Vocabulary:
  gnExportEntries / gnRefreshVocab / gnSetVocabSync
  gnSaveSnapshot / gnLoadSnapshot / gnWindowStats
Utility:
  gnTest()
  gnHybridRebuild()
---
## NAPI RULES -- DO NOT VIOLATE
1. Never use sed -i '/fn_name/,+Nd' -- cuts mid-function, orphans braces
2. Never append #[napi] without checking grep -n "^#[napi]" first
3. Never re-add a function that already exists -- check grep -n "fn gn_name" first
4. After every build verify exports:
   node -e "const gn=require('./gn-native.linux-x64-gnu.node');console.log(Object.keys(gn).sort())"
5. use napi_derive::napi appears exactly once -- grep -c to verify
6. Never use JsObject/Object::new() in async napi -- not Send
7. flate2 must be in gn-node/Cargo.toml for inflate in bindings
8. Always use absolute path:
   path.resolve(__dirname, '../gn-node/gn-native.linux-x64-gnu.node')
9. lattice-ui has type:module in package.json -- use .cjs for CommonJS scripts
10. Add single function, build, verify exports, test -- then next function
11. Never use sed on JS files with template literals -- use Python str.replace()
    Always assert old string exists in file before replace or get silent failures
---
## VERIFIED BENCHMARK RESULTS (April 16 2026 -- CANONICAL)

### BENCH METHODOLOGY -- READ THIS BEFORE RUNNING ANY BENCH
The correct bench is in ~/glasik-core/bench/gcdict_all_corpora.js
Rules that must be followed every time:
  - Warm: first 200 chunks through gnCompressSplitBatch (8 at a time)
  - Test: chunks 200-700 in batches of 8
  - GN function: gnCompressSplitBatch -- NOT gnCompressSplit, NOT gnSplitRaw alone
  - brotli comparison: PER-MESSAGE (one brotli call per chunk, NOT per batch)
    brotli/batch is brotli best case -- not production fair, report separately
  - gzip comparison: per-message, level 6
  - GCdict: train from literal residue of first 400 msgs, last 32KB as preset dict
  - Corpus extraction: use content field only -- NOT str(turn) which includes
    JSON metadata wrapper and inflates numbers by ~89B/turn
  - 4 corpora minimum, report all. Never cite single-corpus numbers as general.

### GN + GCdict (HEADLINE -- beats brotli on ALL corpora)
Architecture: gnSplitRaw -> deflate(tok) + deflate_with_dict(lit, gcdict)
GCdict = last 32KB of literal residue from 400 training messages
Verified April 16 2026:
  WildChat:    2.324x  vs brotli/msg +3.8%   GCdict gain over split: +8.2%
  ShareGPT:    2.693x  vs brotli/msg +9.4%   GCdict gain over split: +5.8%
  LMSYS:       2.504x  vs brotli/msg +7.5%   GCdict gain over split: +4.7%
  Ubuntu-IRC:  1.889x  vs brotli/msg +60.6%  GCdict gain over split: +11.1%
GCdict file: ~/.openclaw/gn-gcdict-wildchat-32k.bin (32KB, WildChat-trained)

### GN split-stream b=8 baseline (no GCdict)
Verified April 16 2026 (per-message brotli comparison):
  WildChat:    2.148x  vs gzip/msg +5.6%   vs brotli/msg -4.6%
  ShareGPT:    2.520x  vs gzip/msg +14.8%  vs brotli/msg +3.2%
  LMSYS:       2.359x  vs gzip/msg +13.1%  vs brotli/msg +2.6%
  Ubuntu-IRC:  ~2.5x   vs gzip/msg +60%    vs brotli/msg +47%

### CORRECTED numbers (old fudged numbers -- do not use)
Previous sessions reported 2.833x WildChat, 3.004x LMSYS -- WRONG.
Cause: str(turn) extraction included JSON metadata wrapper adding ~89B/turn.
Correct extraction: t.content field only (clean message text).
Corrected numbers are the canonical ones listed above.

### GCdict on general compression (Silesia + Canterbury)
GN+GCdict vs brotli per-message:
  mean: +46.5%  19/20 files positive  20/20 vs gzip
  xml: +127.8%  osdb: +122.1%  samba: +85.2%
  Only loser: kennedy.xls (-7%) -- binary Excel

### Production metrics (OpenClaw live, April 13 2026)
  Messages: 3,570   Avg ratio: 2.404x   Max: 10.878x
  Total saved: 2,440.9KB
  Source: ~/.openclaw/gn-metrics.csv

### Latency
  gnCompressSplitBatch p50: 0.3-0.5ms  p99: 0.06-0.08ms
  FractalCompressor p50: 1.89ms  p99: 2.41ms
  VTC v3: ~2.0ms per call
---
## GCDICT ARCHITECTURE
How it works:
  1. gnSplitRaw([buf]) -- returns [tok_ids_buf, literals_buf]
     NOTE: input MUST be an array, not a bare Buffer
  2. deflateRaw(tok, {level:6}) -- token stream plain deflate
  3. deflateRaw(lit, {level:6, dictionary: gcdict}) -- literal with preset dict
  4. Frame: tok_compressed + lit_compressed
  5. Decoder needs same gcdict -- it is NOT embedded in frame (stateful)

Key insight: GN AC tokenization finds cross-session vocabulary patterns.
Literal residue compressed with conversation history as preset dict.
History dict exploits LLM conversation self-referentiality.
Per-message brotli has no session context. GN+GCdict does. That is the edge.

Training a GCdict:
  - Load corpus JSONL, extract t.content (NOT str(turn))
  - Warm vocab: gnCompressSplitBatch first 200 msgs
  - Collect literal residues: gnSplitRaw([buf])[1] for each training msg
  - Concatenate all lit residues, take last 32KB as dict
  - Save to ~/.openclaw/gn-gcdict-<corpus>-32k.bin
  - Per-corpus dicts outperform single global dict

Bench script: ~/glasik-core/bench/gcdict_all_corpora.js
---
## FRACTAL FRAME FORMAT
[1B shard_type][2B pairs_len LE][2B l3_ser_len LE][l3_ser bytes][deflated pairs][deflated literals]
gnGetPairs returns inflated pairs:
  [(2B lit_count LE)(1B tok_id)...][2B trailing_lits LE]
  pair count = (raw.length - 2) / 3

VTC FORMAT (v3 -- LIVE as of April 14 2026)
  gnCompressFractalWithVtc returns: "VTC-v3-<64 hex chars>" (len=71)
  SHA256(shard_type || session_id || canonical_pairs || literal_hash || sequence_fingerprint)
  Same content + same session = same VTC always
  8/8 fidelity tests pass: deterministic, collision-resistant, lossless

SHARD TYPES
  user_intent / assistant_response / system_message /
  code_block / tool_call / tool_result / generic
---
## GN ARCHITECTURE (L0-L4)
L0: Universal (pre-trained, 20k entries, IDs 1-63)
L1: Domain (per shard type, IDs 64-127, promotes from L2 at freq>=8)
L2: Session (sliding window, IDs 128-191, excluded from AC)
L3: Chunk (ephemeral, IDs 192-254, built per-chunk, stored in frame)
L4: Meta (fractal dictionary compression, persistence)
FractalCompressor = orchestration layer binding L0-L3 into self-contained shard.
76/76 tests passing (confirmed April 14 2026).
---
## SNAPSHOTS AND TRAINING DATA
gn-window.snapshot (auto-loads at napi startup):
  Path: ~/.openclaw/gn-window.snapshot
  Size: 2.2MB  Entries: 20,000 (saturated)
  Training: 200k chunks, 4 corpora interleaved
gn-gcdict-wildchat-32k.bin:
  Path: ~/.openclaw/gn-gcdict-wildchat-32k.bin
  Size: 32KB  Trained: WildChat literal residue, 400 msgs
Training corpora:
  ShareGPT V3:  ~/Downloads/sharegpt-v3.json
  WildChat:     ~/Downloads/WildChat/*.parquet
  LMSYS:        ~/Downloads/lmsys/*.parquet
  Ubuntu IRC:   ~/Downloads/Ubuntu-dialogue-corpus/*.csv
Bench JSONL (regenerate if missing after reboot):
  /tmp/wildchat_turns.jsonl
  /tmp/sharegpt_turns.jsonl
  /tmp/lmsys_turns.jsonl
  /tmp/ubuntu_irc_turns.jsonl
---
## LATTICE UI
Location: ~/glasik-core/lattice-ui/
Stack: Vite + React + Three.js
Key files:
  generate-lattice.cjs        -- data generator (MUST be .cjs not .js)
  public/lattice.json         -- generated graph with real pairs data
  src/App.jsx                 -- 3D crystal renderer
  src/App.snapshot.GOLD.jsx   -- safe rollback point
Run:
  cd ~/glasik-core/lattice-ui
  node generate-lattice.cjs
  npm run dev   -- starts at localhost:5173
Editing rules:
  - Always use Python str.replace() -- NEVER sed on App.jsx (template literals)
  - After edit: check localhost:5173 immediately
  - If broken: cp src/App.snapshot.GOLD.jsx src/App.jsx then restart
  - Take new GOLD snapshot after any working state: cp src/App.jsx src/App.snapshot.GOLD.jsx
  - generate-lattice.cjs uses gnCompressFractalWithVtc + gnGetPairs for real pairs
  - lattice.json stores pairs per VTC node -- real crystal geometry not synthetic
---
## OPENCLAW GN INTEGRATION
napi addon: /home/boot/glasik-core/gn-node/gn-native.linux-x64-gnu.node
Config: ~/.openclaw/openclaw.json
HEARTBEAT.md: ~/.openclaw/workspace/HEARTBEAT.md (exact case)
OLLAMA_HOST for offline: 192.168.0.2:11434
DB State (April 16 2026):
  Main: /home/boot/glasik-core/data/gn-shards.db (~14,500+ shards)
    gn-claude-8878b9e3: 8358 shards @ 4.124x avg
    claude-session-1: 6190 shards @ 4.288x avg
  Bug-bounty DB: /home/boot/.openclaw/workspace/projects/bug-bounty/dashboard/data/gn-shards.db
    DIFFERENT SCHEMA -- no shard_type, has embedding/importance_score
    DO NOT merge directly -- RangeError on schema mismatch
Chrome Extension v2.2.0:
  ID: llnhjgamebhfelehailfofpcfbjnjgdb
  After ANY edit: reload chrome://extensions/ + Ctrl+Shift+R on claude.ai
Dashboard:
  /home/boot/.openclaw/gn-dashboard-server.js
  Edit with Python str.replace() ONLY -- never sed (template literal JS)
  Always assert old string in content before replace
Lattice UI: cd ~/glasik-core/lattice-ui && node generate-lattice.cjs && npm run dev
---
## PAPER STATUS
  Title: GN: Domain-Adaptive Lossless Compression for LLM Conversation Streams
  File: ~/Desktop/GN-Split-Stream/docs/GN_PAPER_FINAL.md
  PDF:  ~/Desktop/GN-Split-Stream/docs/GN_PAPER_FINAL.pdf
  Status: READY -- blocked on arXiv cs.IR endorsement (code 7HWUBA)
  Alternative venues: SSRN, TechRxiv, Zenodo (no endorsement needed)
  Paper corrections needed:
    - Abstract: update to "beats brotli on all 4 corpora with GCdict"
    - Results table: update to GCdict numbers (April 16 canonical)
    - Ubuntu-IRC line: +60.6% vs brotli (not +2% or +28%)
---
## ARTICLE STATUS (dev.to/atomsrkuul)
  Article 1: Sliding window tokenizer -- LIVE
  Article 2: Aho-Corasick O(n) matching -- LIVE
  Article 3: First verified benchmarks vs gzip -- LIVE
  Article 4: The ESCAPE Byte Problem (split-stream, beats brotli) -- LIVE April 13
  Article 5: Fractal sharding + crystal architecture -- PENDING
  Article 6: GCdict -- beats brotli on ALL corpora -- PENDING (write this next)
  Article 7: GCdict on general compression -- PENDING
---
## COMPLETED THIS SESSION (April 16 2026)
- GN+GCdict verified beating brotli on all 4 corpora
- gn-auto-compressor.js: self-optimizing engine built and wired
  - Per-namespace, zero configuration
  - Checkpoint at msg 50, recheck every 200
  - Every 10th msg held out for honest validation
  - Persists to ~/.openclaw/gn-dicts/
  - Fractal path included in recheck at msg 200+
  - Verified on real WildChat: +6.3% at checkpoint, activates correctly
- gn-shards.js wired to auto-compressor
  - Namespace from sessionId prefix (claude-abc -> claude)
  - mode field in shard record reflects actual path used
- Bench scripts saved permanently:
  ~/glasik-core/bench/gcdict_all_corpora.js
  ~/glasik-core/bench/gcdict_claude_bench.js
  /tmp/gcdict_falloff.js (regenerate if lost)

## PENDING (priority order)
1. NLNet talking points -- April 29 2026 deadline (#2026-06-023) URGENT
2. arXiv endorsement -- code 7HWUBA, cs.IR, 3+ cs papers required
3. Paper corrections -- update all numbers to April 16 canonical
4. Wire GCdict into gn-split-stream-encoder.js compressBatch()
5. Wire GCdict into gn-context-compressor.js _flushBatch()
6. Article 6 dev.to -- GCdict beats brotli on all corpora
7. OpenClaw DB -- unified schema, wire bug-bounty server.js
8. Per-namespace vocabulary routing (per-shard-type L0)
9. WildChat-optimized snapshot routing
10. Multi-seed verification of GCdict numbers (4 corpora x 3 seeds)
---
## GIT WORKFLOW
glasik-core:
  cd ~/glasik-core && git add -A && git commit -m "msg" && git push origin master
Glasik-Workspace:
  cd ~/.openclaw/workspace && git add -A && git commit -m "msg" && git push origin master
---
## CONTACT / ENDORSEMENT REQUEST TEMPLATE
Subject: arXiv endorsement request for compression paper
Hi,
I'm an independent researcher working on domain-adaptive lossless compression
for LLM conversation streams. I have a paper ready to submit to arXiv cs.IR
and need an endorsement from a qualified author.
The paper: "GN: Domain-Adaptive Lossless Compression for LLM Conversation Streams"
GitHub: github.com/atomsrkuul/glasik-core
Key result: beats brotli on all 4 corpora with GCdict architecture.
ShareGPT +9.4%, LMSYS +7.5%, WildChat +3.8%, IRC +60.6%.
Endorsement code: 7HWUBA
Robert Rider / atomsrkuul@gmail.com
---
## HANDING OFF TO NEW SESSION
Strengths by model:
  CC:     architecture analysis, root cause, layer attribution
  GPro:   code generation, Rust implementation, bug fixing
  Glasik: wiring JS, file management, benchmarking, OpenClaw integration
  Glasik weakness: error prone on complex Rust, tends to use stubs

Rules for new session:
  1. Read this file top to bottom before touching anything
  2. Run bench first to verify current state before any changes:
     node --no-lazy ~/glasik-core/bench/gcdict_all_corpora.js 2>&1 | grep -v "GN-NATIVE\|GN-FRACTAL\|GN-CTX"
  3. Check /tmp JSONL files exist before benching -- regenerate if missing
  4. Never cite numbers from memory -- always run the bench
  5. The canonical bench is brotli PER-MESSAGE not per-batch
  6. Do not use sed on JS or Rust files -- Python str.replace() only
  7. Do not produce commands requiring manual file editing -- CLI only
  8. Add one function at a time, build, verify exports, test, then next
  9. Before any napi work: grep -n "fn gn_name" to check existence first
  10. After any napi build: verify exports with node -e "Object.keys(require(...))"
