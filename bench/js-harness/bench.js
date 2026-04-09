const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const gn = require('../../gn-node/gn-native.linux-x64-gnu.node');

function percentile(arr, p) {
  const s = [...arr].sort((a,b)=>a-b);
  return s[Math.min(s.length-1, Math.floor(p/100*s.length))];
}

function loadCorpus(name) {
  return fs.readFileSync(path.join(__dirname,'../corpora',name),'utf8')
    .trim().split('\n').filter(Boolean).map(l=>Buffer.from(JSON.parse(l)));
}

function bench(name, corpus, chunks, batchSize, fn) {
  const lats = [];
  let bytesIn=0, bytesOut=0;
  // warmup
  for (let i=0;i<Math.min(20,chunks.length);i++) fn(chunks.slice(i,i+1));
  const t0 = process.hrtime.bigint();
  for (let i=0;i<chunks.length;i+=batchSize) {
    const batch = chunks.slice(i,i+batchSize);
    const s = process.hrtime.bigint();
    const out = fn(batch);
    lats.push(Number(process.hrtime.bigint()-s)/1e3);
    bytesIn += batch.reduce((s,c)=>s+c.length,0);
    bytesOut += out.reduce((s,r)=>s+r.length,0);
  }
  const ms = Number(process.hrtime.bigint()-t0)/1e6;
  return {
    name, corpus, batch_size: batchSize, chunks: chunks.length,
    bytes_in: bytesIn, bytes_out: bytesOut,
    ratio: (bytesIn/Math.max(1,bytesOut)).toFixed(3),
    per_chunk_us: (ms*1000/chunks.length).toFixed(1),
    p50_us: percentile(lats,50).toFixed(1),
    p95_us: percentile(lats,95).toFixed(1),
    p99_us: percentile(lats,99).toFixed(1),
    throughput_mb_s: ((bytesIn/1024/1024)/(ms/1000)).toFixed(1),
  };
}

function main() {
  const corpora = {
    small:  loadCorpus('small_chat.jsonl'),
    medium: loadCorpus('medium_chat.jsonl'),
  };

  const runners = {
    'gzip-6':       b => b.map(c=>zlib.deflateRawSync(c,{level:6})),
    'gzip-1':       b => b.map(c=>zlib.deflateRawSync(c,{level:1})),
    'gn_l1':        b => b.map(c=>gn.gnCompress(c)),
    'gn_fast':      b => b.map(c=>gn.gnCompressFastSync(c)),
    'gn_tl':        b => b.map(c=>gn.gnCompressTl(c)),
    'gn_parallel':  b => gn.gnCompressBatch(b),
    'napi_noop':    b => b,  // boundary overhead only
  };

  const results = [];
  for (const [cname, chunks] of Object.entries(corpora)) {
    for (const bs of [1, 8, 32]) {
      for (const [rname, fn] of Object.entries(runners)) {
        results.push(bench(rname, cname, chunks, bs, fn));
      }
    }
  }

  // Table
  const cols = ['name','corpus','batch_size','ratio','per_chunk_us','p50_us','p95_us','p99_us','throughput_mb_s'];
  console.log(cols.join('\t'));
  for (const r of results) console.log(cols.map(c=>r[c]).join('\t'));

  // Layer attribution
  console.log('\n=== LAYER ATTRIBUTION (batch_size=1) ===');
  for (const corpus of ['small','medium']) {
    const g = (name) => results.find(r=>r.name===name&&r.corpus===corpus&&r.batch_size===1);
    const noop = parseFloat(g('napi_noop').per_chunk_us);
    const gzip = parseFloat(g('gzip-6').per_chunk_us);
    const tl   = parseFloat(g('gn_tl').per_chunk_us);
    const par  = parseFloat(results.find(r=>r.name==='gn_parallel'&&r.corpus===corpus&&r.batch_size===32).per_chunk_us);
    console.log(`\n${corpus} (avg ${(chunks=>chunks.reduce((s,c)=>s+c.length,0)/chunks.length|0)(corpora[corpus])}B):`);
    console.log(`  napi boundary:        ${noop.toFixed(1)}µs`);
    console.log(`  gzip-6:               ${gzip.toFixed(1)}µs`);
    console.log(`  GN thread-local:      ${tl.toFixed(1)}µs`);
    console.log(`  GN parallel batch32:  ${par.toFixed(1)}µs`);
    console.log(`  GN core estimate:     ${(tl-noop).toFixed(1)}µs`);
    console.log(`  gzip ratio:           ${g('gzip-6').ratio}x`);
    console.log(`  GN ratio:             ${g('gn_tl').ratio}x`);
  }
}
main();
