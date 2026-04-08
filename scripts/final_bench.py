import glasik_core as gc, gzip, brotli, json, random, csv, time
import pyarrow.parquet as pq
from pathlib import Path
from multiprocessing import Pool, cpu_count

def br6(c): return len(brotli.compress(c, quality=6))
def gz6(c): return len(gzip.compress(c, 6))

def load_sharegpt(n, seed):
    random.seed(seed)
    sgpt = json.loads(Path("/home/boot/Downloads/sharegpt-v3.json").read_bytes())
    chunks = []
    for conv in sgpt:
        for turn in conv.get("conversations", []):
            v = turn.get("value", "")
            if 200 < len(v) < 3000: chunks.append(v.encode())
    random.shuffle(chunks)
    return chunks[:n]

def load_wildchat(n, seed):
    random.seed(seed)
    chunks = []
    for f in sorted(Path("/home/boot/Downloads/WildChat").glob("*.parquet")):
        t = pq.read_table(f)
        for i in range(len(t)):
            for turn in t['conversation'][i].as_py():
                c = turn.get('content','')
                if 200 < len(c) < 3000: chunks.append(c.encode())
        if len(chunks) >= n*2: break
    random.shuffle(chunks)
    return chunks[:n]

def load_lmsys(n, seed):
    random.seed(seed)
    chunks = []
    for f in sorted(Path("/home/boot/Downloads/lmsys").glob("*.parquet")):
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

def load_ubuntu_fused(n, seed, target=800):
    random.seed(seed)
    raw = []
    with open("/home/boot/Downloads/Ubuntu-dialogue-corpus/dialogueText.csv") as f:
        reader = csv.DictReader(f)
        for row in reader:
            text = row.get('text','').strip()
            if text: raw.append(text)
            if len(raw) >= n*20: break
    fused = []
    buf = ""
    for msg in raw:
        if len(buf) + len(msg) < target:
            buf += " | " + msg if buf else msg
        else:
            if buf: fused.append(buf.encode())
            buf = msg
    if buf: fused.append(buf.encode())
    random.shuffle(fused)
    return fused[:n]

def run(chunks, br_sizes, gz_sizes, pk=3):
    s2    = gc.GlasikSlidingV2()
    s2_sd = gc.GlasikSlidingV2.with_bundled_dict()
    s3_sd = gc.GlasikSlidingV2.with_bundled_dict()
    raw=gz=br=l2=l2sd=l3sd=0
    history=[]
    for i,c in enumerate(chunks):
        raw+=len(c); gz+=gz_sizes[i]; br+=br_sizes[i]
        l2+=len(s2.compress(c))
        l2sd+=len(s2_sd.compress(c))
        if len(c)>=200 and history:
            for w in history[-pk:]: s3_sd.compress(w)
        l3sd+=len(s3_sd.compress(c))
        history.append(c)
    return raw,gz,br,l2,l2sd,l3sd

SEEDS=[42,123,777]
N=2000
CORPORA=[
    ("ShareGPT",      load_sharegpt,    3),
    ("WildChat",      load_wildchat,    2),
    ("LMSYS",         load_lmsys,       3),
    ("Ubuntu-IRC",    load_ubuntu_fused,2),
]

print("=== GN FINAL REPEATABILITY BENCHMARK ===", flush=True)
print("4 corpora x 3 seeds x n=2000, brotli-6, L3+static", flush=True)
print(f"\n{'Corpus':<12} {'Seed':>4}  {'gzip':>7} {'brotli':>7} {'L2':>7} {'L2+sd':>7} {'L3+sd':>7}  {'L3+sdvBr':>12}", flush=True)
print("-"*82, flush=True)

summary = {c:{k:[] for k in ['gz','br','l2','l2sd','l3sd']} for c,_,__ in CORPORA}

with Pool(cpu_count()) as pool:
    for cname, loader, pk in CORPORA:
        all_chunks = {seed: loader(N, seed) for seed in SEEDS}
        all_c = []
        for seed in SEEDS: all_c.extend(all_chunks[seed])
        print(f"{cname:<12} precomputing ({cpu_count()} cores) avg={sum(len(c) for c in all_c)//len(all_c)}B...", flush=True)
        br_all = pool.map(br6, all_c)
        gz_all = pool.map(gz6, all_c)
        br_by_seed = {seed: br_all[i*N:(i+1)*N] for i,seed in enumerate(SEEDS)}
        gz_by_seed = {seed: gz_all[i*N:(i+1)*N] for i,seed in enumerate(SEEDS)}

        for seed in SEEDS:
            t0=time.time()
            raw,gz,br,l2,l2sd,l3sd = run(all_chunks[seed], br_by_seed[seed], gz_by_seed[seed], pk)
            elapsed=time.time()-t0
            l3sdbr = f"-{(1-l3sd/br)*100:.1f}% BEATS" if l3sd<br else f"+{(l3sd/br-1)*100:.1f}% gap"
            print(f"{cname:<12} {seed:>4}  {raw/gz:>7.3f}x {raw/br:>7.3f}x {raw/l2:>7.3f}x {raw/l2sd:>7.3f}x {raw/l3sd:>7.3f}x  {l3sdbr:>12}  ({elapsed:.0f}s)", flush=True)
            summary[cname]['gz'].append(raw/gz)
            summary[cname]['br'].append(raw/br)
            summary[cname]['l2'].append(raw/l2)
            summary[cname]['l2sd'].append(raw/l2sd)
            summary[cname]['l3sd'].append(raw/l3sd)
        print(flush=True)

print("\n=== FINAL SUMMARY (avg 3 seeds, n=2000) ===", flush=True)
print(f"{'Corpus':<12} {'gzip':>7} {'brotli':>7} {'L2':>7} {'L2+sd':>7} {'L3+sd':>7}  {'L3+sdvBr':>12}", flush=True)
print("-"*78, flush=True)
for cname,_,__ in CORPORA:
    s=summary[cname]
    gz=sum(s['gz'])/3; br=sum(s['br'])/3
    l2=sum(s['l2'])/3; l2sd=sum(s['l2sd'])/3; l3sd=sum(s['l3sd'])/3
    l3sdbr = f"-{(1-l3sd/br)*100:.1f}% BEATS" if l3sd>br else f"+{(l3sd/br-1)*100:.1f}% gap"
    print(f"{cname:<12} {gz:>7.3f}x {br:>7.3f}x {l2:>7.3f}x {l2sd:>7.3f}x {l3sd:>7.3f}x  {l3sdbr:>12}", flush=True)
