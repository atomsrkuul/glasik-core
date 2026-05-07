const gn = require('/home/boot/glasik-core/gn-node/gn-native.linux-x64-gnu.node');
const fs = require('fs');
const readline = require('readline');
const zlib = require('zlib');
const Database = require('/home/boot/.openclaw/workspace/node_modules/better-sqlite3');

// Seeded shuffle -- Fisher-Yates with LCG
function seededShuffle(arr, seed) {
  const a = [...arr];
  let s = seed >>> 0;
  const rand = () => { s = (s * 1664525 + 1013904223) >>> 0; return s / 0xFFFFFFFF; };
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

function brotli(buf) {
  return zlib.brotliCompressSync(buf, { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 6 } });
}

async function loadJSONL(path, max) {
  const turns = [];
  try {
    const rl = readline.createInterface({ input: fs.createReadStream(path) });
    for await (const line of rl) {
      if (line.trim()) turns.push(JSON.parse(line));
      if (turns.length >= max) break;
    }
  } catch(e) { console.log('MISSING: ' + path); }
  return turns;
}

function loadClaude() {
  const turns = [];
  try {
    const lines = require('fs').readFileSync('/tmp/claude_turns.jsonl', 'utf8').trim().split('\n');
    lines.forEach(l => { try { turns.push(JSON.parse(l)); } catch(e) {} });
  } catch(e) { console.log('Claude JSONL error: ' + e.message); }
  return turns;
}

async function bench(label, allTurns, seed) {
  if (allTurns.length < 100) { console.log('\n=== ' + label + ' === SKIPPED'); return; }

  const turns = seededShuffle(allTurns, seed);
  const bufs = turns.map(t => Buffer.from(t.content));

  console.log('\n=== ' + label + ' (seed=' + seed + ') ===');
  console.log('Total turns: ' + bufs.length + '  avg size: ' + Math.round(bufs.reduce((s,b)=>s+b.length,0)/bufs.length) + 'B');

  // Warm vocab on first 200
  for (let i = 0; i < 200 && i + 8 <= bufs.length; i += 8) {
    await gn.gnCompressSplitBatch(bufs.slice(i, i + 8));
  }

  // Train GCdict from literal residue of first 400
  const trainBufs = bufs.slice(0, 400);
  const litStreams = [];
  for (const buf of trainBufs) {
    const r = await gn.gnSplitRaw([buf]);
    if (r[1] && r[1].length > 0) litStreams.push(r[1]);
  }
  const gcdict = Buffer.concat(litStreams).slice(-32768);
  console.log('GCdict trained: ' + gcdict.length + 'B from ' + litStreams.length + ' msgs');

  // Bench on next 300
  const testBufs = bufs.slice(400, 700);
  let origTotal = 0, gnComp = 0, dictComp = 0, brComp = 0;
  const gnLatencies = [], dictLatencies = [], brLatencies = [];

  for (let i = 0; i + 8 <= testBufs.length; i += 8) {
    const batch = testBufs.slice(i, i + 8);
    const orig = batch.reduce((s, b) => s + b.length, 0);
    origTotal += orig;

    // GN split latency
    const t0 = process.hrtime.bigint();
    const gnOut = await gn.gnCompressSplitBatch(batch);
    gnLatencies.push(Number(process.hrtime.bigint() - t0) / 1e6);
    gnComp += gnOut.length;

    // GCdict latency
    const t1 = process.hrtime.bigint();
    let dtok = 0, dlit = 0;
    for (const buf of batch) {
      const r = await gn.gnSplitRaw([buf]);
      const tok = r[0] || Buffer.alloc(0);
      const lit = r[1] || Buffer.alloc(0);
      if (tok.length > 0) dtok += Math.min(zlib.deflateRawSync(tok, {level:6}).length, tok.length);
      if (lit.length > 0) {
        try { dlit += Math.min(zlib.deflateRawSync(lit, {level:6, dictionary:gcdict}).length, lit.length); }
        catch(e) { dlit += Math.min(zlib.deflateRawSync(lit, {level:6}).length, lit.length); }
      }
    }
    dictComp += dtok + dlit;
    dictLatencies.push(Number(process.hrtime.bigint() - t1) / 1e6);

    // Brotli latency
    const t2 = process.hrtime.bigint();
    brComp += batch.reduce((s, b) => s + brotli(b).length, 0);
    brLatencies.push(Number(process.hrtime.bigint() - t2) / 1e6);
  }

  const gnR   = origTotal / gnComp;
  const dictR = origTotal / dictComp;
  const brR   = origTotal / brComp;

  const pct = (arr, p) => arr.sort((a,b)=>a-b)[Math.floor(arr.length*p/100)];
  const mbps = (bytes, ms) => ((bytes/1024/1024) / (ms/1000)).toFixed(1);

  const gnTotalMs = gnLatencies.reduce((a,b)=>a+b,0);
  const brTotalMs = brLatencies.reduce((a,b)=>a+b,0);

  console.log('--- COMPRESSION RATIO ---');
  console.log('GN split:    ' + gnR.toFixed(3) + 'x   vs brotli: ' + ((gnR/brR-1)*100).toFixed(1) + '%');
  console.log('GN+GCdict:   ' + dictR.toFixed(3) + 'x   vs brotli: ' + ((dictR/brR-1)*100).toFixed(1) + '%  <-- HEADLINE');
  console.log('brotli-6:    ' + brR.toFixed(3) + 'x');
  console.log('GCdict gain: +' + ((dictR/gnR-1)*100).toFixed(1) + '%');
  console.log('--- LATENCY (per batch of 8) ---');
  console.log('GN split:   p50=' + pct(gnLatencies,50).toFixed(2) + 'ms  p90=' + pct(gnLatencies,90).toFixed(2) + 'ms  p99=' + pct(gnLatencies,99).toFixed(2) + 'ms');
  console.log('brotli:     p50=' + pct(brLatencies,50).toFixed(2) + 'ms  p90=' + pct(brLatencies,90).toFixed(2) + 'ms  p99=' + pct(brLatencies,99).toFixed(2) + 'ms');
  console.log('--- THROUGHPUT ---');
  console.log('GN split:   ' + mbps(origTotal, gnTotalMs) + ' MB/s');
  console.log('brotli:     ' + mbps(origTotal, brTotalMs) + ' MB/s');
}

const SEEDS = [1337, 2718, 31415];

async function verifyRoundTrip(turns, label) {
  const { compress: gnCompress, decompress: gnDecompress } = require('../gn-node/index.js');
  let pass = 0, fail = 0;
  const sample = turns.slice(0, 50);
  for (const t of sample) {
    const orig = Buffer.from(t.content || t.text || '');
    if (!orig.length) continue;
    try {
      const comp = await gnCompress(orig);
      const restored = await gnDecompress(comp);
      if (orig.equals(restored)) pass++;
      else fail++;
    } catch(e) { fail++; }
  }
  console.log(`  Round-trip [${label}]: ${pass}/50 pass, ${fail} fail`);
}

(async () => {
  const wild  = await loadJSONL('/tmp/wildchat_turns.jsonl', 1000);
  const share = await loadJSONL('/tmp/sharegpt_turns.jsonl', 1000);
  const lmsys = await loadJSONL('/tmp/lmsys_turns.jsonl', 1000);
  const irc   = await loadJSONL('/tmp/ubuntu_irc_turns.jsonl', 1000);
  const claude = loadClaude();

  console.log('============================================================');
  console.log('GN BENCHMARK v2 -- Multi-seed with latency + throughput');
  console.log('============================================================');

  for (const seed of SEEDS) {
    await bench('WildChat',   wild,   seed);
    await verifyRoundTrip(wild,   'WildChat');
    await bench('ShareGPT',   share,  seed);
    await verifyRoundTrip(share,  'ShareGPT');
    await bench('LMSYS',      lmsys,  seed);
    await verifyRoundTrip(lmsys,  'LMSYS');
    await bench('Ubuntu-IRC', irc,    seed);
    await verifyRoundTrip(irc,    'Ubuntu-IRC');
    if (claude.length > 100) {
      await bench('Claude', claude, seed);
      await verifyRoundTrip(claude, 'Claude');
    }
    console.log('\n' + '='.repeat(60));
  }
})();

// ---- conversations.json loader (Claude export format) ----
function loadClaudeConversations() {
  const turns = [];
  try {
    const data = JSON.parse(fs.readFileSync('/home/boot/Downloads/Corpora/conversations.json', 'utf8'));
    for (const conv of data) {
      for (const msg of (conv.chat_messages || [])) {
        let text = msg.text || '';
        if (!text && Array.isArray(msg.content)) {
          text = msg.content.map(b => b.text || '').join(' ');
        }
        text = text.trim();
        if (text.length > 10) turns.push({ content: text });
      }
    }
  } catch(e) { console.log('conversations.json error: ' + e.message); }
  console.log('Loaded conversations.json: ' + turns.length + ' turns');
  return turns;
}
