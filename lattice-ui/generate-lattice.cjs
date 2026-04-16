const fs = require('fs');
const path = require('path');
const Database = require('better-sqlite3');
const gn = require(path.resolve(__dirname, '../gn-node/gn-native.linux-x64-gnu.node'));

async function generateLattice(rows, label) {
  const graph = {};
  const sessionPrev = {};
  let skipped = 0;

  for (const row of rows) {
    const stype = row.shard_type || 'generic';
    const session = row.session_id || 'default';
    const date = row.created_at ? row.created_at.slice(0, 10) : 'unknown';
    try {
      const buf = Buffer.from(row.content);
      const vtc = await gn.gnCompressFractalWithVtc(buf, stype, session);
      const raw = await gn.gnGetPairs(buf, stype, session);
      const pairs = [];
      for (let i = 0; i + 2 < raw.length - 2; i += 3) {
        pairs.push({ lit: raw[i] | (raw[i+1] << 8), tok: raw[i+2] });
      }
      if (!graph[vtc]) {
        graph[vtc] = {
          next: {}, count: 0, pairs,
          type: stype, session, date,
          ratio: row.compression_ratio || 1.0,
        };
      }
      graph[vtc].count++;
      const prev = sessionPrev[session];
      if (prev && graph[prev]) {
        graph[prev].next[vtc] = (graph[prev].next[vtc] || 0) + 1;
      }
      sessionPrev[session] = vtc;
    } catch(e) { skipped++; }
  }

  const nodes = Object.keys(graph).length;
  const edges = Object.values(graph).reduce((a, v) => a + Object.keys(v.next).length, 0);
  console.log(label + ': ' + nodes + ' nodes, ' + edges + ' edges, ' + skipped + ' skipped');
  return graph;
}

(async () => {
  const MAIN_DB = '/home/boot/glasik-core/data/gn-shards.db';
  const OPENCLAW_DB = '/home/boot/.openclaw/workspace/projects/bug-bounty/dashboard/data/gn-shards.db';

  const db = new Database(MAIN_DB, { readonly: true });
  const cols = db.pragma('table_info(shards)').map(c => c.name);
  const hasCreated = cols.includes('created_at');
  const hasRatio = cols.includes('compression_ratio');

  let sel = 'SELECT content, session_id, shard_type';
  if (hasCreated) sel += ', created_at';
  if (hasRatio) sel += ', compression_ratio';
  sel += ' FROM shards WHERE content IS NOT NULL AND length(content) > 20 ORDER BY RANDOM() LIMIT 500';

  const allRows = db.prepare(sel).all();

  // ALL lattice
  const allGraph = await generateLattice(allRows, 'ALL');
  fs.writeFileSync(path.resolve(__dirname, 'public/lattice.json'), JSON.stringify(allGraph, null, 2));

  // GN sessions (gn-claude-* prefix)
  const gnRows = allRows.filter(r => r.session_id && (r.session_id.startsWith('gn-') || r.session_id.startsWith('gn-claude')));
  const gnGraph = await generateLattice(gnRows, 'GN');
  fs.writeFileSync(path.resolve(__dirname, 'public/lattice-gn.json'), JSON.stringify(gnGraph, null, 2));

  // GLASIK sessions (claude-session-* prefix)
  const glasikRows = allRows.filter(r => r.session_id && r.session_id.startsWith('claude-'));
  const glasikGraph = await generateLattice(glasikRows, 'GLASIK');
  fs.writeFileSync(path.resolve(__dirname, 'public/lattice-glasik.json'), JSON.stringify(glasikGraph, null, 2));

  db.close();

  // OPENCLAW DB
  if (fs.existsSync(OPENCLAW_DB)) {
    const odb = new Database(OPENCLAW_DB, { readonly: true });
    const ocols = odb.pragma('table_info(shards)').map(c => c.name);
    let osel = 'SELECT content';
    if (ocols.includes('session_id')) osel += ', session_id';
    if (ocols.includes('created_at')) osel += ', created_at';
    if (ocols.includes('compression_ratio')) osel += ', compression_ratio';
    osel += ' FROM shards WHERE content IS NOT NULL AND length(content) > 20 LIMIT 150';
    const orows = odb.prepare(osel).all().map(r => ({ ...r, shard_type: 'generic' }));
    odb.close();
    const ocGraph = await generateLattice(orows, 'OPENCLAW');
    fs.writeFileSync(path.resolve(__dirname, 'public/lattice-openclaw.json'), JSON.stringify(ocGraph, null, 2));
  }

  console.log('All lattice JSONs generated.');
})();