import glasik_core as gc, gzip, brotli, json, random, struct, re
import numpy as np
from pathlib import Path
from multiprocessing import Pool, cpu_count

def br6(c): return len(brotli.compress(c, quality=6))

# Load GN embeddings
def load_embeddings(path='/home/boot/.openclaw/workspace/scripts/gn-embed.bin'):
    with open(path, 'rb') as f:
        f.read(4); f.read(4)
        vocab_size = struct.unpack('<I', f.read(4))[0]
        dim = struct.unpack('<I', f.read(4))[0]
        word2idx = {}
        vectors = []
        for i in range(vocab_size):
            wlen = struct.unpack('<B', f.read(1))[0]
            word = f.read(wlen).decode('utf-8', errors='replace')
            vec = np.frombuffer(f.read(dim * 4), dtype=np.float32).copy()
            word2idx[word] = i
            vectors.append(vec)
    return word2idx, np.array(vectors, dtype=np.float32)

STOPS = {'the','and','to','of','in','is','for','that','with','this',
         'are','was','has','have','but','not','from','they','their'}

def embed(text, word2idx, vectors):
    tokens = [t for t in re.findall(r"[a-z][a-z0-9']{2,}", text.lower())
              if t not in STOPS]
    vecs = [vectors[word2idx[t]] for t in tokens if t in word2idx]
    if not vecs: return np.zeros(vectors.shape[1], dtype=np.float32)
    v = np.mean(vecs, axis=0)
    norm = np.linalg.norm(v)
    return v / norm if norm > 0 else v

def top_k_similar(query_vec, history_vecs, k=3):
    if not history_vecs: return []
    sims = np.dot(np.array(history_vecs), query_vec)
    top_k = np.argsort(sims)[-k:][::-1]
    return [i for i in top_k if sims[i] > 0.3]  # threshold

print("Loading embeddings...", flush=True)
word2idx, vectors = load_embeddings()
print(f"Loaded {len(word2idx)} vectors dim={vectors.shape[1]}", flush=True)

# Load ShareGPT
sgpt = json.loads(Path("/home/boot/Downloads/sharegpt-v3.json").read_bytes())
chunks = []
for conv in sgpt:
    for turn in conv.get("conversations", []):
        v = turn.get("value", "")
        if 200 < len(v) < 3000: chunks.append(v.encode())
random.seed(42)
random.shuffle(chunks)
sample = chunks[:2000]
print(f"ShareGPT: {len(sample)} chunks avg {sum(len(c) for c in sample)//len(sample)}B", flush=True)

print("Precomputing brotli-6...", flush=True)
with Pool(cpu_count()) as pool:
    br_sizes = pool.map(br6, sample)

print(f"\n{'seed':>4} {'br':>7} {'L3seq':>7} {'L3sem':>7}  {'L3seqvBr':>10} {'L3semvBr':>10}", flush=True)
print("-"*60, flush=True)

for seed in [42, 123, 777]:
    random.seed(seed)
    idx = list(range(len(sample)))
    random.shuffle(idx)
    s = [sample[i] for i in idx]
    br_s = [br_sizes[i] for i in idx]

    # L3 sequential (pk=3)
    s_seq = gc.GlasikSlidingV2.with_bundled_dict()
    # L3 semantic
    s_sem = gc.GlasikSlidingV2.with_bundled_dict()

    raw=br=l3seq=l3sem=0
    history_chunks = []
    history_vecs = []

    for i, c in enumerate(s):
        raw += len(c)
        br  += br_s[i]

        # Sequential L3: warm with last 3
        if history_chunks:
            for w in history_chunks[-3:]: s_seq.compress(w)
        l3seq += len(s_seq.compress(c))

        # Semantic L3: warm with top-3 most similar
        text = c.decode('utf-8', errors='replace')
        qvec = embed(text, word2idx, vectors)
        if history_vecs:
            top_idx = top_k_similar(qvec, history_vecs, k=3)
            for ti in top_idx:
                s_sem.compress(history_chunks[ti])
        l3sem += len(s_sem.compress(c))

        history_chunks.append(c)
        history_vecs.append(qvec)

    seq_br = f"-{(1-l3seq/br)*100:.1f}%B" if l3seq<br else f"+{(l3seq/br-1)*100:.1f}%"
    sem_br = f"-{(1-l3sem/br)*100:.1f}%B" if l3sem<br else f"+{(l3sem/br-1)*100:.1f}%"
    print(f"{seed:>4} {raw/br:>7.3f}x {raw/l3seq:>7.3f}x {raw/l3sem:>7.3f}x  {seq_br:>10} {sem_br:>10}", flush=True)
