import glasik_core as gc, gzip, brotli, json, time, random
import pyarrow.parquet as pq
from pathlib import Path

def load_wildchat(n, seed):
    random.seed(seed)
    chunks = []
    for f in sorted(Path("/home/boot/Downloads/Corpora/WildChat").glob("*.parquet")):
        t = pq.read_table(f)
        for i in range(len(t)):
            for turn in t['conversation'][i].as_py():
                c = turn.get('content', '')
                if 200 < len(c) < 3000: chunks.append(c.encode())
        if len(chunks) >= n*2: break
    random.shuffle(chunks)
    return chunks[:n]

def load_lmsys(n, seed):
    random.seed(seed)
    chunks = []
    for f in sorted(Path("/home/boot/Downloads/Corpora/lmsys").glob("*.parquet")):
        t = pq.read_table(f)
        for i in range(len(t)):
            conv = t['conversation'][i].as_py()
            if isinstance(conv, list):
                for turn in conv:
                    c = turn.get('content','') if isinstance(turn,dict) else ''
                    if 200 < len(c) < 3000: chunks.append(c.encode())
        if len(chunks) >= n*2: break
    random.shuffle(chunks)
    return chunks[:n]

def load_sharegpt(n, seed):
    random.seed(seed)
    sgpt = json.loads(Path("/home/boot/Downloads/Corpora/sharegpt-v3.json").read_bytes())
    chunks = []
    for conv in sgpt:
        for turn in conv.get("conversations", []):
            v = turn.get("value", "")
            if 200 < len(v) < 3000: chunks.append(v.encode())
    random.shuffle(chunks)
    return chunks[:n]

def run_bench(chunks):
    milestones = [100, 500, 1000, 2000]
    raw_cum = gz_cum = br_cum = l1_cum = l2_cum = l3_cum = 0
    s2 = gc.GlasikSlidingV2()
    s3 = gc.GlasikSlidingV2()
    history = []
    results = {}
    for i, c in enumerate(chunks):
        raw_cum += len(c)
        gz_cum  += len(gzip.compress(c, 6))
        br_cum  += len(brotli.compress(c, quality=6))
        l1_cum  += len(gc.gn_compress(c))
        l2_cum  += len(s2.compress(c))
        if len(c) >= 200 and history:
            for w in history[-3:]: s3.compress(w)
        l3_cum  += len(s3.compress(c))
        history.append(c)
        if (i+1) in milestones:
            results[i+1] = {
                'raw': raw_cum, 'gz': gz_cum, 'br': br_cum,
                'l1': l1_cum,  'l2': l2_cum,  'l3': l3_cum,
                'w2': s2.stats()[0], 'w3': s3.stats()[0]
            }
    return results

SEEDS = [42, 123, 777]
MILESTONES = [500, 1000, 2000]
N = 2000

loaders = {
    "ShareGPT": load_sharegpt,
    "WildChat":  load_wildchat,
    "LMSYS":    load_lmsys,
}

print("=== GN L1/L2/L3 Benchmark: 3 Corpora x 3 Seeds ===\n")
print(f"{'Corpus':<10} {'Seed':>4} {'n':>5}  {'gzip':>7} {'brotli':>7} {'L1':>7} {'L2':>7} {'L3':>7}  {'L2vL1':>7} {'L3vL1':>7}  {'L2vBr':>8} {'L3vBr':>8}")
print("-"*105)

summary = {}  # corpus -> {l1, l2, l3, gz, br} lists at n=2000

for cname, loader in loaders.items():
    summary[cname] = {'l1':[], 'l2':[], 'l3':[], 'gz':[], 'br':[]}
    for seed in SEEDS:
        print(f"Loading {cname} seed={seed}...", flush=True)
        chunks = loader(N, seed)
        res = run_bench(chunks)
        for m in MILESTONES:
            r = res[m]
            raw = r['raw']
            l2_br = "BEATS" if r['l2'] < r['br'] else f"+{(r['l2']/r['br']-1)*100:.1f}%"
            l3_br = "BEATS" if r['l3'] < r['br'] else f"+{(r['l3']/r['br']-1)*100:.1f}%"
            print(f"{cname:<10} {seed:>4} {m:>5}  {raw/r['gz']:>7.3f}x {raw/r['br']:>7.3f}x {raw/r['l1']:>7.3f}x {raw/r['l2']:>7.3f}x {raw/r['l3']:>7.3f}x  +{(r['l1']/r['l2']-1)*100:.1f}%  +{(r['l1']/r['l3']-1)*100:.1f}%  {l2_br:>8} {l3_br:>8}", flush=True)
        # Store n=2000 for summary
        r2k = res[2000]
        raw2k = r2k['raw']
        summary[cname]['l1'].append(raw2k/r2k['l1'])
        summary[cname]['l2'].append(raw2k/r2k['l2'])
        summary[cname]['l3'].append(raw2k/r2k['l3'])
        summary[cname]['gz'].append(raw2k/r2k['gz'])
        summary[cname]['br'].append(raw2k/r2k['br'])
    print()

print("\n=== Summary at n=2000 (avg across 3 seeds) ===")
print(f"{'Corpus':<10} {'gzip':>7} {'brotli':>7} {'L1':>7} {'L2':>7} {'L3':>7}  {'L2vL1':>7} {'L3vL1':>7}  {'L2vBr':>9} {'L3vBr':>9}")
print("-"*90)
for cname in loaders:
    s = summary[cname]
    gz  = sum(s['gz'])/3
    br  = sum(s['br'])/3
    l1  = sum(s['l1'])/3
    l2  = sum(s['l2'])/3
    l3  = sum(s['l3'])/3
    l2_br = "BEATS" if l2 > br else f"+{(1-l2/br)*100:.1f}% gap"
    l3_br = "BEATS" if l3 > br else f"+{(1-l3/br)*100:.1f}% gap"
    print(f"{cname:<10} {gz:>7.3f}x {br:>7.3f}x {l1:>7.3f}x {l2:>7.3f}x {l3:>7.3f}x  +{(l2/l1-1)*100:.1f}%  +{(l3/l1-1)*100:.1f}%  {l2_br:>9} {l3_br:>9}")
