import glasik_core as gc, gzip, brotli, json, random
import pyarrow.parquet as pq
from pathlib import Path

def load_parquet(parquet_dir, max_chunks=6000, seed=42):
    random.seed(seed)
    chunks = []
    for f in sorted(Path(parquet_dir).glob("*.parquet")):
        t = pq.read_table(f)
        for i in range(len(t)):
            conv = t['conversation'][i].as_py()
            for turn in conv:
                content = turn.get('content', '').strip()
                if 100 < len(content) < 4000:
                    chunks.append(content.encode())
        if len(chunks) >= max_chunks:
            break
    random.shuffle(chunks)
    return chunks[:max_chunks]

def bench(chunks, label):
    slider = gc.GlasikSlidingV2()
    raw = gz = br = v2 = 0
    results = []
    for i, c in enumerate(chunks):
        raw += len(c)
        gz  += len(gzip.compress(c, compresslevel=6))
        br  += len(brotli.compress(c, quality=6))
        v2  += len(slider.compress(c))
        if i+1 in (500, 1000, 2000, 5000):
            e, b = slider.stats()
            beat = "BEATS" if v2 < br else f"+{(v2/br-1)*100:.1f}%"
            results.append(f"  {label} n={i+1:>5} GN={raw/v2:.3f}x gz={raw/gz:.3f}x br={raw/br:.3f}x win={e} {beat}")
    return results

print("=== GN SlidingV2 Repeatability Benchmark ===")
print("3 runs per corpus with shuffled order\n")

corpora = [
    ("ShareGPT", "/home/boot/Downloads/sharegpt-v3.json", "json"),
    ("WildChat",  "/home/boot/Downloads/WildChat", "parquet"),
    ("LMSYS",    "/home/boot/Downloads/lmsys", "parquet"),
]

for seed in [42, 123, 777]:
    print(f"--- Seed {seed} ---")
    for name, path, fmt in corpora:
        if fmt == "json":
            random.seed(seed)
            sgpt = json.loads(Path(path).read_bytes())
            chunks = []
            for conv in sgpt:
                for turn in conv.get("conversations", []):
                    v = turn.get("value", "")
                    if 100 < len(v) < 4000: chunks.append(v.encode())
            random.shuffle(chunks)
            chunks = chunks[:6000]
        else:
            chunks = load_parquet(path, 6000, seed)
        for line in bench(chunks, name):
            print(line)
    print()
