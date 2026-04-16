const gn = require('/home/boot/glasik-core/gn-node/gn-native.linux-x64-gnu.node');
const fs = require('fs');
const zlib = require('zlib');

function brotli(buf) {
  return zlib.brotliCompressSync(buf, {
    params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 6 }
  });
}

function percentile(arr, p) {
  const sorted = [...arr].sort((a, b) => a - b);
  const idx = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.max(0, idx)];
}

function loadClaude(path) {
  console.log('Loading ' + path + '...');
  const raw = JSON.parse(fs.readFileSync(path, 'utf8'));

  const turns = [];
  const convos = Array.isArray(raw) ? raw : (raw.conversations || raw.chats || Object.values(raw));

  for (const convo of convos) {
    const messages = convo.messages || convo.chat_messages || [];
    for (const msg of messages) {
      const content =
        typeof msg.content === 'string' ? msg.content :
        Array.isArray(msg.content) ? msg.content.map(c => typeof c === 'string' ? c : (c.text || '')).join('') :
        '';
      if (content.length > 20) {
        turns.push({
          content,
          role: msg.role || msg.sender || 'unknown'
        });
      }
    }
  }

  console.log('Loaded ' + turns.length + ' turns from ' + convos.length + ' conversations');
  return turns;
}

async function bench(label, turns) {
  console.log('\n' + '='.repeat(60));
  console.log('=== ' + label + ' ===');
  console.log('='.repeat(60));

  if (turns.length < 100) {
    console.log('SKIPPED -- insufficient data (' + turns.length + ' turns)');
    return;
  }

  const bufs = turns.map(t => Buffer.from(t.content));
  const avgSize = Math.round(bufs.reduce((s, b) => s + b.length, 0) / bufs.length);
  console.log('Total turns: ' + bufs.length + '  avg size: ' + avgSize + 'B');

  // --- Warm vocab ---
  const warmCount = Math.min(200, Math.floor(bufs.length * 0.4));
  for (let i = 0; i < warmCount && i + 8 <= bufs.length; i += 8) {
    await gn.gnCompressSplitBatch(bufs.slice(i, i + 8));
  }
  console.log('Warmed on ' + warmCount + ' messages');

  // --- Train GCdict ---
  const trainBufs = bufs.slice(0, Math.min(400, Math.floor(bufs.length * 0.5)));
  const litStreams = [];
  for (const buf of trainBufs) {
    const r = await gn.gnSplitRaw([buf]);
    if (r[1] && r[1].length > 0) litStreams.push(r[1]);
  }
  const trainCorpus = Buffer.concat(litStreams);
  const gcdict = trainCorpus.slice(-32768);
  console.log('GCdict trained: ' + gcdict.length + 'B from ' + litStreams.length + ' msgs');

  // --- Test ---
  const testStart = Math.floor(bufs.length * 0.5);
  const testBufs = bufs.slice(testStart, testStart + 300);
  if (testBufs.length < 8) {
    console.log('SKIPPED -- not enough test data after split');
    return;
  }

  let origTotal = 0;
  let gnComp = 0, dictComp = 0, gzComp = 0, brComp = 0;

  // Latency tracking
  const gnLatencies = [], dictLatencies = [], brLatencies = [], gzLatencies = [];

  // Throughput tracking
  let gnBytes = 0, dictBytes = 0;
  let gnMs = 0, dictMs = 0;

  for (let i = 0; i + 8 <= testBufs.length; i += 8) {
    const batch = testBufs.slice(i, i + 8);
    const orig = batch.reduce((s, b) => s + b.length, 0);
    origTotal += orig;

    // GN split-stream
    const t0 = performance.now();
    const gnOut = await gn.gnCompressSplitBatch(batch);
    const gnT = performance.now() - t0;
    gnComp += gnOut.length;
    gnLatencies.push(gnT);
    gnBytes += orig;
    gnMs += gnT;

    // GCdict path
    const t1 = performance.now();
    let dtok = 0, dlit = 0;
    for (const buf of batch) {
      const r = await gn.gnSplitRaw([buf]);
      const tok = r[0] || Buffer.alloc(0);
      const lit = r[1] || Buffer.alloc(0);
      if (tok.length > 0) {
        const tc = zlib.deflateRawSync(tok, { level: 6 });
        dtok += Math.min(tc.length, tok.length);
      }
      if (lit.length > 0) {
        try {
          const lc = zlib.deflateRawSync(lit, { level: 6, dictionary: gcdict });
          dlit += Math.min(lc.length, lit.length);
        } catch(e) {
          const lc = zlib.deflateRawSync(lit, { level: 6 });
          dlit += Math.min(lc.length, lit.length);
        }
      }
    }
    const dictT = performance.now() - t1;
    dictComp += dtok + dlit;
    dictLatencies.push(dictT);
    dictBytes += orig;
    dictMs += dictT;

    // gzip per-message
    const t2 = performance.now();
    gzComp += batch.reduce((s, b) => s + zlib.gzipSync(b, { level: 6 }).length, 0);
    gzLatencies.push(performance.now() - t2);

    // brotli per-message
    const t3 = performance.now();
    brComp += batch.reduce((s, b) => s + brotli(b).length, 0);
    brLatencies.push(performance.now() - t3);
  }

  const gnR    = origTotal / gnComp;
  const dictR  = origTotal / dictComp;
  const gzR    = origTotal / gzComp;
  const brR    = origTotal / brComp;

  const gnThroughput   = (gnBytes / 1024 / 1024) / (gnMs / 1000);
  const dictThroughput = (dictBytes / 1024 / 1024) / (dictMs / 1000);
  const brThroughput   = (origTotal / 1024 / 1024) /
    (brLatencies.reduce((a, b) => a + b, 0) / 1000);
  const gzThroughput   = (origTotal / 1024 / 1024) /
    (gzLatencies.reduce((a, b) => a + b, 0) / 1000);

  console.log('\n--- COMPRESSION RATIO ---');
  console.log('orig:              ' + origTotal + 'B  (' + (origTotal/1024).toFixed(0) + 'KB)');
  console.log('GN split:          ' + gnR.toFixed(3) + 'x' +
    '   vs gzip: ' + ((gnR/gzR-1)*100).toFixed(1) + '%' +
    '   vs brotli: ' + ((gnR/brR-1)*100).toFixed(1) + '%');
  console.log('GN + GCdict:       ' + dictR.toFixed(3) + 'x' +
    '   vs gzip: ' + ((dictR/gzR-1)*100).toFixed(1) + '%' +
    '   vs brotli: ' + ((dictR/brR-1)*100).toFixed(1) + '%  <-- HEADLINE');
  console.log('gzip-6/msg:        ' + gzR.toFixed(3) + 'x');
  console.log('brotli-6/msg:      ' + brR.toFixed(3) + 'x');
  console.log('GCdict gain:       +' + ((dictR/gnR-1)*100).toFixed(1) + '%');

  console.log('\n--- LATENCY (per batch of 8) ---');
  console.log('GN split:   p50=' + percentile(gnLatencies, 50).toFixed(2) + 'ms' +
    '  p90=' + percentile(gnLatencies, 90).toFixed(2) + 'ms' +
    '  p99=' + percentile(gnLatencies, 99).toFixed(2) + 'ms');
  console.log('GN+GCdict:  p50=' + percentile(dictLatencies, 50).toFixed(2) + 'ms' +
    '  p90=' + percentile(dictLatencies, 90).toFixed(2) + 'ms' +
    '  p99=' + percentile(dictLatencies, 99).toFixed(2) + 'ms');
  console.log('brotli/msg: p50=' + percentile(brLatencies, 50).toFixed(2) + 'ms' +
    '  p90=' + percentile(brLatencies, 90).toFixed(2) + 'ms' +
    '  p99=' + percentile(brLatencies, 99).toFixed(2) + 'ms');
  console.log('gzip/msg:   p50=' + percentile(gzLatencies, 50).toFixed(2) + 'ms' +
    '  p90=' + percentile(gzLatencies, 90).toFixed(2) + 'ms' +
    '  p99=' + percentile(gzLatencies, 99).toFixed(2) + 'ms');

  console.log('\n--- THROUGHPUT ---');
  console.log('GN split:   ' + gnThroughput.toFixed(1) + ' MB/s');
  console.log('GN+GCdict:  ' + dictThroughput.toFixed(1) + ' MB/s');
  console.log('brotli/msg: ' + brThroughput.toFixed(1) + ' MB/s');
  console.log('gzip/msg:   ' + gzThroughput.toFixed(1) + ' MB/s');

  // Role breakdown if available
  const userTurns = turns.filter(t => t.role === 'human' || t.role === 'user');
  const asstTurns = turns.filter(t => t.role === 'assistant');
  if (userTurns.length > 0 && asstTurns.length > 0) {
    console.log('\n--- ROLE BREAKDOWN ---');
    console.log('User turns:      ' + userTurns.length +
      '  avg ' + Math.round(userTurns.reduce((s,t)=>s+t.content.length,0)/userTurns.length) + 'B');
    console.log('Assistant turns: ' + asstTurns.length +
      '  avg ' + Math.round(asstTurns.reduce((s,t)=>s+t.content.length,0)/asstTurns.length) + 'B');
  }
}

(async () => {
  const claudeTurns = loadClaude('/home/boot/Downloads/conversations.json');
  await bench('Claude Conversations', claudeTurns);
})();
