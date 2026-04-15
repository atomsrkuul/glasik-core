import fs from 'fs';
import Database from 'better-sqlite3';

const db = new Database('/home/boot/.openclaw/workspace/glasik-shards.db');

const shards = db.prepare(`
  SELECT shard_id, shard_type, compression_ratio, pairs_data 
  FROM shards 
  ORDER BY created_at ASC
`).all();

console.log(`Found ${shards.length} shards`);

const graph = {};
shards.forEach((shard, idx) => {
  const pairs = shard.pairs_data ? JSON.parse(shard.pairs_data) : [];
  graph[shard.shard_id] = {
    count: idx + 1,
    ratio: shard.compression_ratio,
    type: shard.shard_type || 'batch',
    pairs: pairs.slice(0, 20),
  };
});

fs.writeFileSync(
  './lattice.json',
  JSON.stringify(graph, null, 2)
);

console.log(`✓ Written ${Object.keys(graph).length} shards to lattice.json`);
