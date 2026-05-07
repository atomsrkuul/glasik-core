#!/usr/bin/env python3
"""
Train GN static dictionary from general text corpora (Silesia, Canterbury, enwik8).
Blends with existing LLM dict for universal bootstrap.
"""
import glasik_core as gc, json, struct
from pathlib import Path

SILESIA_FILES = ['dickens','webster','reymont','samba','mr','osdb','ooffice']
CHUNK_SIZE = 65536
N_CHUNKS_PER_FILE = 50  # 50 * 64KB = 3.2MB per file

print("Loading general corpora...", flush=True)
training = []

for fname in SILESIA_FILES:
    p = Path(f'/home/boot/silesia/{fname}')
    if not p.exists(): continue
    data = p.read_bytes()
    chunks = [data[i:i+CHUNK_SIZE] for i in range(0, len(data), CHUNK_SIZE)]
    training.extend(chunks[:N_CHUNKS_PER_FILE])
    print(f"  {fname}: {len(chunks[:N_CHUNKS_PER_FILE])} chunks", flush=True)

# Canterbury text files
for fname in ['alice29.txt','asyoulik.txt','lcet10.txt','plrabn12.txt','xargs.1']:
    p = Path(f'/home/boot/canterbury/{fname}')
    if not p.exists(): continue
    data = p.read_bytes()
    chunks = [data[i:i+CHUNK_SIZE] for i in range(0, len(data), CHUNK_SIZE)]
    training.extend(chunks)
    print(f"  {fname}: {len(chunks)} chunks", flush=True)

# enwik8 - sample 200 chunks spread across file
p = Path('/home/boot/enwik/enwik8')
if p.exists():
    data = p.read_bytes()
    step = len(data) // 200
    training.extend([data[i:i+CHUNK_SIZE] for i in range(0, len(data), step)][:200])
    print(f"  enwik8: 200 chunks sampled", flush=True)

print(f"\nTotal training chunks: {len(training)}", flush=True)

# Run sliding window
sw = gc.GlasikSlidingV2()
for i, chunk in enumerate(training):
    sw.ingest_fast(bytes(chunk))
    if i % 100 == 0: print(f"  ingested {i}/{len(training)}", flush=True, end='\r')

print("\nExporting dict...", flush=True)
raw_json = sw.export_dict_json(); _d = json.loads(raw_json); _lst = _d if isinstance(_d, list) else _d.get("entries", list()); raw = list(map(lambda e: (bytes(e.get("b", list())), e.get("f",0), e.get("s",0)), _lst))
entries = sorted(raw, key=lambda x: -x[2])[:5000]  # top 5000 by saving

# Write GNSD binary
out = Path('/tmp/gn_general_dict.json')
out.write_text(json.dumps([{'b': list(b), 'f': int(f), 's': int(s)} for b,f,s in entries]))
print(f"Written {len(entries)} entries to {out}", flush=True)

# Also write binary format
def write_gnsd(entries, path):
    with open(path, 'wb') as f:
        f.write(b'GNSD')
        f.write(struct.pack('<I', 1))  # version
        f.write(struct.pack('<I', len(entries)))
        for b, freq, saving in entries:
            f.write(bytes([len(b)]))
            f.write(bytes(b))
            f.write(struct.pack('<Q', int(freq)))
            f.write(struct.pack('<Q', int(saving)))
    print(f"Written binary dict: {path} ({Path(path).stat().st_size} bytes)")

write_gnsd(entries, '/tmp/gn_general_dict.bin')
