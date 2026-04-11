const fs = require('fs');
const path = require('path');

const gn = require(
  path.resolve(__dirname, '../gn-node/gn-native.linux-x64-gnu.node')
);

(async () => {

  const graph = {};

  const inputs = [
    'user: login failed',
    'assistant: retry login',
    'user: login failed',
    'assistant: retry login',
    'user: login success'
  ];

  let prev = null;

  for (const input of inputs) {

    const buf = Buffer.from(input);

    // 🔥 use available function
    const frame = await gn.gnCompressFractal(
      buf,
      'user_intent',
      'graph'
    );

    // 🔥 derive stable identity from frame
    const vtc = await gn.gnCompressFractalWithVtc(buf, 'user_intent', 'graph');

    const raw = await gn.gnGetPairs(
      buf,
      'user_intent',
      'graph'
    );

    console.log("RAW LEN:", raw.length);

    const pairs = [];

    for (let i = 0; i + 2 < raw.length - 2; i += 3) {
      const lit = raw[i] | (raw[i + 1] << 8);
      const tok = raw[i + 2];
      pairs.push({ lit, tok });
    }

    console.log("PAIRS:", pairs.length);

    if (!graph[vtc]) {
      graph[vtc] = {
        next: {},
        count: 0,
        pairs
      };
    }

    graph[vtc].count++;

    if (prev) {
      graph[prev].next[vtc] =
        (graph[prev].next[vtc] || 0) + 1;
    }

    prev = vtc;
  }

  fs.writeFileSync(
    path.resolve(__dirname, 'public/lattice.json'),
    JSON.stringify(graph, null, 2)
  );

  console.log('✔ lattice.json generated (using fractal)');
})();
