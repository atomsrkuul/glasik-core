#!/usr/bin/env python3
"""
Train GN static dictionary from LLM + IRC corpora.
Runs GN sliding window on large corpus, captures top entries by cumulative saving.
Output: /tmp/gn_static_dict.json -- top 5000 entries
"""
import glasik_core as gc, json, csv, random
import pyarrow.parquet as pq
from pathlib import Path
from collections import defaultdict

N_LLM = 10000   # chunks per LLM corpus
N_IRC = 5000    # IRC chunks

print("Loading corpora...", flush=True)

# ShareGPT
chunks = []
sgpt = json.loads(Path("/home/boot/Downloads/sharegpt-v3.json").read_bytes())
for conv in sgpt:
    for turn in conv.get("conversations", []):
        v = turn.get("value", "")
        if 100 < len(v) < 4000: chunks.append(v.encode())
print(f"ShareGPT: {len(chunks[:N_LLM])} chunks", flush=True)
training = chunks[:N_LLM]

# WildChat
wc_chunks = []
for f in sorted(Path("/home/boot/Downloads/WildChat").glob("*.parquet")):
    t = pq.read_table(f)
    for i in range(len(t)):
        for turn in t['conversation'][i].as_py():
            c = turn.get('content','')
            if 100 < len(c) < 4000: wc_chunks.append(c.encode())
    if len(wc_chunks) >= N_LLM: break
training += wc_chunks[:N_LLM]
print(f"WildChat: {len(wc_chunks[:N_LLM])} chunks", flush=True)

# LMSYS
lm_chunks = []
for f in sorted(Path("/home/boot/Downloads/lmsys").glob("*.parquet")):
    t = pq.read_table(f)
    for i in range(len(t)):
        conv = t['conversation'][i].as_py()
        if isinstance(conv, list):
            for turn in conv:
                c = turn.get('content','') if isinstance(turn,dict) else ''
                if 100 < len(c) < 4000: lm_chunks.append(c.encode())
    if len(lm_chunks) >= N_LLM: break
training += lm_chunks[:N_LLM]
print(f"LMSYS: {len(lm_chunks[:N_LLM])} chunks", flush=True)

# Ubuntu IRC
irc_chunks = []
with open("/home/boot/Downloads/Ubuntu-dialogue-corpus/dialogueText.csv") as f:
    reader = csv.DictReader(f)
    for row in reader:
        text = row.get('text','').strip()
        if 50 < len(text) < 4000: irc_chunks.append(text.encode())
        if len(irc_chunks) >= N_IRC: break
training += irc_chunks[:N_IRC]
print(f"Ubuntu IRC: {len(irc_chunks[:N_IRC])} chunks", flush=True)

print(f"\nTotal training chunks: {len(training)}", flush=True)
random.seed(42)
random.shuffle(training)

# Run sliding window to saturation
print("Training sliding window...", flush=True)
slider = gc.GlasikSlidingV2()
for i, chunk in enumerate(training):
    slider.compress(chunk)
    if (i+1) % 5000 == 0:
        entries, batches = slider.stats()
        print(f"  {i+1}/{len(training)} chunks, window={entries} entries", flush=True)

entries, batches = slider.stats()
print(f"\nFinal window: {entries} entries after {batches} batches", flush=True)

# Export window state
version, exported = slider.export_dict()
print(f"Exported {len(exported)} entries (version={version})", flush=True)

# Sort by cumulative saving, take top 5000
exported_sorted = sorted(exported, key=lambda x: x[2], reverse=True)
top = exported_sorted[:5000]

# Serialize to JSON
static_dict = {
    "version": 1,
    "entries": [
        {
            "bytes": list(b),
            "freq": int(f),
            "saving": int(s)
        }
        for b, f, s in top
    ]
}

out = Path("/tmp/gn_static_dict.json")
out.write_text(json.dumps(static_dict, indent=2))
print(f"\nSaved {len(top)} entries to {out}", flush=True)
print(f"File size: {out.stat().st_size/1024:.1f}KB", flush=True)

# Show top 20 entries
print("\nTop 20 entries by saving:")
for i, (b, f, s) in enumerate(top[:20]):
    try:
        text = bytes(b).decode('utf-8', errors='replace')
    except:
        text = repr(bytes(b))
    print(f"  {i+1:>3}. saving={s:>8} freq={f:>8} | {repr(text)[:60]}")
