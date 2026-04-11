const fs = require('fs');
const path = require('path');
const gn = require(
  path.resolve(__dirname, '../gn-node/gn-native.linux-x64-gnu.node')
);

(async () => {
  const graph = {};

  const conversations = [
    { type: 'system_message',       text: 'You are a helpful assistant with expertise in network security and compression systems.' },
    { type: 'user_intent',          text: 'user: how do I implement JWT authentication in Node.js?' },
    { type: 'assistant_response',   text: 'assistant: JWT authentication requires signing a token with a secret key and verifying it on each request.' },
    { type: 'user_intent',          text: 'user: what is the best compression algorithm for LLM context data?' },
    { type: 'assistant_response',   text: 'assistant: domain-adaptive compression like GN outperforms gzip and brotli on LLM conversation streams.' },
    { type: 'code_block',           text: 'function authenticate(token) { return jwt.verify(token, process.env.SECRET); }' },
    { type: 'user_intent',          text: 'user: login failed please retry' },
    { type: 'assistant_response',   text: 'assistant: please check your credentials and try again' },
    { type: 'user_intent',          text: 'user: login failed please retry' },
    { type: 'assistant_response',   text: 'assistant: your account may be locked after multiple failed attempts' },
    { type: 'user_intent',          text: 'user: login success thank you' },
    { type: 'tool_call',            text: '{"tool":"search","query":"compression benchmark results","params":{"limit":10}}' },
    { type: 'tool_result',          text: '{"results":[{"ratio":2.5,"method":"GN"},{"ratio":2.1,"method":"gzip"}]}' },
    { type: 'code_block',           text: 'const compress = async (data) => gnCompressSplitBatch([Buffer.from(data)]);' },
    { type: 'user_intent',          text: 'user: explain fractal compression to me' },
    { type: 'assistant_response',   text: 'assistant: fractal compression finds self-similar patterns across multiple scales of the data.' },
    { type: 'system_message',       text: 'System context updated. Memory shard loaded. Vocabulary tier L1 active for code_block domain.' },
    { type: 'user_intent',          text: 'user: what is a virtual time crystal?' },
    { type: 'assistant_response',   text: 'assistant: a virtual time crystal is a deterministic structure whose identity is derived from its compression shape.' },
    { type: 'code_block',           text: 'fn compress_shard(data: &[u8], shard_type: &str) -> Vec<u8> { fractal.compress(data) }' },
    { type: 'user_intent',          text: 'user: how does vocabulary promotion work in GN?' },
    { type: 'assistant_response',   text: 'assistant: L3 patterns promote to L2 at frequency 3, L2 promotes to L1 at frequency 50.' },
    { type: 'tool_call',            text: '{"tool":"compress","input":"hello world","shard_type":"user_intent"}' },
    { type: 'tool_result',          text: '{"vtc":"VTC-v1-abc123","ratio":2.47,"pairs":29}' },
    { type: 'user_intent',          text: 'user: show me the benchmark results for ShareGPT corpus' },
    { type: 'assistant_response',   text: 'assistant: GN split b=8 achieves 2.49-2.52x on ShareGPT, beating brotli by 2 percent.' },
  ];

  let prev = null;

  for (const { type, text } of conversations) {
    const buf = Buffer.from(text);
    const vtc = await gn.gnCompressFractalWithVtc(buf, type, 'session_main');
    const raw = await gn.gnGetPairs(buf, type, 'session_main');

    const pairs = [];
    for (let i = 0; i + 2 < raw.length - 2; i += 3) {
      pairs.push({ lit: raw[i] | (raw[i+1] << 8), tok: raw[i+2] });
    }

    if (!graph[vtc]) {
      graph[vtc] = { next: {}, count: 0, pairs, type };
    }
    graph[vtc].count++;

    if (prev) {
      graph[prev].next[vtc] = (graph[prev].next[vtc] || 0) + 1;
    }
    prev = vtc;
  }

  fs.writeFileSync(
    path.resolve(__dirname, 'public/lattice.json'),
    JSON.stringify(graph, null, 2)
  );

  const nodeCount = Object.keys(graph).length;
  const edgeCount = Object.values(graph).reduce((a, n) => a + Object.keys(n.next).length, 0);
  console.log('lattice.json generated:', nodeCount, 'nodes,', edgeCount, 'edges');

})();
