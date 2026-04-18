const gn = require('/home/boot/glasik-core/gn-node/gn-native.linux-x64-gnu.node');
const fs = require('fs');
const readline = require('readline');
const zlib = require('zlib');

function brotli(buf) {
  return zlib.brotliCompressSync(buf, {
    params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 6 }
  });
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

async function bench(label, turns) {
  if (turns.length < 100) { console.log('\n=== ' + label + ' === SKIPPED (insufficient data)'); return; }
  console.log('\n=== ' + label + ' ===');
  const bufs = turns.map(t => Buffer.from(t.content));

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
  const trainCorpus = Buffer.concat(litStreams);
  const gcdict = trainCorpus.slice(-32768);
  console.log('GCdict trained: ' + gcdict.length + 'B from ' + litStreams.length + ' msgs');

  // Bench on next 300
  const testBufs = bufs.slice(400, 700);
  let origTotal = 0, gnComp = 0, dictComp = 0, brComp = 0;

  for (let i = 0; i + 8 <= testBufs.length; i += 8) {
    const batch = testBufs.slice(i, i + 8);
    const orig = batch.reduce((s, b) => s + b.length, 0);
    origTotal += orig;

    const gnOut = await gn.gnCompressSplitBatch(batch);
    gnComp += gnOut.length;

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
    dictComp += dtok + dlit;
    brComp += batch.reduce((s, b) => s + brotli(b).length, 0);
  }

  const gnR   = origTotal / gnComp;
  const dictR = origTotal / dictComp;
  const brR   = origTotal / brComp;

  console.log('GN split:    ' + gnR.toFixed(3) + 'x   vs brotli/msg: ' + ((gnR/brR-1)*100).toFixed(1) + '%');
  console.log('GN+GCdict:   ' + dictR.toFixed(3) + 'x   vs brotli/msg: ' + ((dictR/brR-1)*100).toFixed(1) + '%  <-- HEADLINE');
  console.log('brotli/msg:  ' + brR.toFixed(3) + 'x');
  console.log('GCdict gain: +' + ((dictR/gnR-1)*100).toFixed(1) + '%');
}

(async () => {
  const wild  = await loadJSONL('/tmp/wildchat_turns.jsonl', 800);
  const share = await loadJSONL('/tmp/sharegpt_turns.jsonl', 800);
  const lmsys = await loadJSONL('/tmp/lmsys_turns.jsonl', 800);
  const irc   = await loadJSONL('/tmp/ubuntu_irc_turns.jsonl', 800);

  await bench('WildChat',  wild);
  await bench('ShareGPT',  share);
  await bench('LMSYS',     lmsys);
  await bench('Ubuntu-IRC', irc);
})();
