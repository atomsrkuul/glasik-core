require('dotenv').config({ path: '/root/gn-api/.env' });
const express = require('express');
const helmet = require('helmet');
const rateLimit = require('express-rate-limit');
const crypto = require('crypto');
const bcrypt = require('bcrypt');
const Database = require('better-sqlite3');
const stripe = require('stripe')(process.env.STRIPE_SECRET_KEY);
const validator = require('validator');

// Load GN engine
const gn = require('/root/gn-api/gn-native.linux-x64-gnu.node');
const GNSplitStreamEncoder = require('/root/gn-api/gn-split-stream-encoder');
const encoder = new GNSplitStreamEncoder({ windowSize: 20000 });

gn.gnLoadSnapshot('/root/gn-api/gn-window.snapshot')
  .then(() => console.log('L0 snapshot loaded'))
  .catch(e => console.error('snapshot failed:', e.message));

const app = express();
app.set('trust proxy', 1);
const PORT = process.env.PORT || 3000;

// Encryption helpers
const ENC_KEY = crypto.scryptSync(process.env.STRIPE_SECRET_KEY, 'glasik-salt', 32);
function encrypt(text) {
  const iv = crypto.randomBytes(16);
  const cipher = crypto.createCipheriv('aes-256-cbc', ENC_KEY, iv);
  return iv.toString('hex') + ':' + cipher.update(text, 'utf8', 'hex') + cipher.final('hex');
}
function decrypt(text) {
  try {
    const [iv, enc] = text.split(':');
    const decipher = crypto.createDecipheriv('aes-256-cbc', ENC_KEY, Buffer.from(iv, 'hex'));
    return decipher.update(enc, 'hex', 'utf8') + decipher.final('utf8');
  } catch(e) { return null; }
}

// DB setup
const db = new Database('/root/gn-api/keys.db');
db.exec(`CREATE TABLE IF NOT EXISTS api_keys (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key_hash TEXT UNIQUE NOT NULL,
  key_prefix TEXT NOT NULL,
  customer_id TEXT,
  email_enc TEXT,
  plan TEXT DEFAULT 'free',
  active INTEGER DEFAULT 1,
  trial_ends_at TEXT,
  stripe_subscription_id TEXT,
  mb_used_this_month REAL DEFAULT 0,
  mb_limit REAL DEFAULT 100,
  created_at TEXT DEFAULT (datetime('now')),
  last_used TEXT
)`);
db.exec(`CREATE TABLE IF NOT EXISTS usage_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key_prefix TEXT,
  mb_compressed REAL,
  endpoint TEXT,
  created_at TEXT DEFAULT (datetime('now'))
)`);
db.exec('CREATE INDEX IF NOT EXISTS idx_hash ON api_keys(key_hash)');
db.exec('CREATE INDEX IF NOT EXISTS idx_prefix ON api_keys(key_prefix)');

// Security middleware
app.use(helmet({
  contentSecurityPolicy: false,
}));

// Rate limiting
const apiLimiter = rateLimit({
  windowMs: 60 * 1000,
  max: 60,
  message: { error: 'Too many requests' },
  standardHeaders: true,
  legacyHeaders: false,
});

const authLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 10,
  message: { error: 'Too many auth attempts' },
});

app.use('/compress', apiLimiter);
app.use('/decompress', apiLimiter);
app.use('/signup', authLimiter);

// Hash API key
function hashKey(key) {
  return crypto.createHash('sha256').update(key).digest('hex');
}

// Auth middleware
function authMiddleware(req, res, next) {
  const key = req.headers['x-api-key'];
  if (!key || !key.startsWith('gn_')) {
    return res.status(401).json({ error: 'Valid API key required (x-api-key header)' });
  }
  const hash = hashKey(key);
  const row = db.prepare('SELECT * FROM api_keys WHERE key_hash = ? AND active = 1').get(hash);
  if (!row) return res.status(401).json({ error: 'Invalid API key' });

  // Check trial expiry
  if (row.trial_ends_at && new Date(row.trial_ends_at) < new Date()) {
    if (!row.stripe_subscription_id) {
      return res.status(402).json({
        error: 'Trial expired',
        message: 'Add a payment method to continue',
        upgrade_url: 'https://glasik.mooo.com/#pricing'
      });
    }
  }

  // Check free tier limit
  if (row.plan === 'free' && row.mb_used_this_month >= row.mb_limit) {
    return res.status(429).json({
      error: 'Free tier limit reached',
      message: `${row.mb_limit}MB monthly limit exceeded`,
      upgrade_url: 'https://glasik.mooo.com/#pricing'
    });
  }

  db.prepare(`UPDATE api_keys SET last_used = datetime('now') WHERE key_hash = ?`).run(hash);
  req.apiKey = row;
  next();
}

function trackUsage(keyPrefix, mb, endpoint) {
  db.prepare('INSERT INTO usage_log (key_prefix, mb_compressed, endpoint) VALUES (?, ?, ?)').run(keyPrefix, mb, endpoint);
  db.prepare('UPDATE api_keys SET mb_used_this_month = mb_used_this_month + ? WHERE key_prefix = ?').run(mb, keyPrefix);
}

// Stripe webhook -- raw body
app.post('/webhook/stripe', express.raw({ type: 'application/json' }), async (req, res) => {
  const sig = req.headers['stripe-signature'];
  let event;
  try {
    event = stripe.webhooks.constructEvent(req.body, sig, process.env.STRIPE_WEBHOOK_SECRET);
  } catch(e) {
    return res.status(400).send(`Webhook error: ${e.message}`);
  }

  switch(event.type) {
    case 'checkout.session.completed': {
      const session = event.data.object;
      const meta = session.metadata || {};
      if (meta.api_key_prefix) {
        const hash = meta.api_key_hash;
        db.prepare(`UPDATE api_keys SET
          customer_id = ?,
          stripe_subscription_id = ?,
          plan = ?,
          mb_limit = ?,
          trial_ends_at = NULL,
          active = 1
          WHERE key_hash = ?`).run(
          session.customer,
          session.subscription,
          meta.plan || 'starter',
          meta.plan === 'pro' ? 25000 : 5000,
          hash
        );
        console.log(`Upgraded key ${meta.api_key_prefix} to ${meta.plan}`);
      }
      break;
    }
    case 'customer.subscription.deleted': {
      const sub = event.data.object;
      db.prepare(`UPDATE api_keys SET
        stripe_subscription_id = NULL,
        plan = 'free',
        mb_limit = 100
        WHERE customer_id = ?`).run(sub.customer);
      console.log(`Downgraded customer ${sub.customer} to free`);
      break;
    }
    case 'invoice.payment_failed': {
      const inv = event.data.object;
      console.log(`Payment failed for ${inv.customer}`);
      break;
    }
  }
  res.json({ received: true });
});

app.use(express.json({ limit: '50mb' }));

// Status
app.get('/', (req, res) => {
  res.json({ status: 'ok', service: 'GN Compression API', version: '1.0.0' });
});

// Signup -- creates free trial key
app.post('/signup', async (req, res) => {
  const { email } = req.body;
  if (!email || !validator.isEmail(email)) {
    return res.status(400).json({ error: 'Valid email required' });
  }

  // Generate API key -- shown only once
  const rawKey = 'gn_' + crypto.randomBytes(24).toString('hex');
  const hash = hashKey(rawKey);
  const prefix = rawKey.slice(0, 12);
  const emailEnc = encrypt(email);
  const trialEnds = new Date(Date.now() + 14 * 24 * 60 * 60 * 1000).toISOString();

  try {
    db.prepare(`INSERT INTO api_keys
      (key_hash, key_prefix, email_enc, plan, mb_limit, trial_ends_at)
      VALUES (?, ?, ?, 'trial', 500, ?)`
    ).run(hash, prefix, emailEnc, trialEnds);

    res.json({
      success: true,
      api_key: rawKey,
      message: 'Save this key — it will not be shown again',
      trial_ends: trialEnds,
      mb_included: 500,
      docs: 'https://glasik.mooo.com/docs'
    });
  } catch(e) {
    if (e.message.includes('UNIQUE')) {
      return res.status(409).json({ error: 'Key collision — try again' });
    }
    res.status(500).json({ error: 'Signup failed' });
  }
});

// Stripe checkout -- upgrade from trial/free to paid
app.post('/create-checkout', authMiddleware, async (req, res) => {
  const { plan } = req.body;
  const priceId = plan === 'pro' ? process.env.PRICE_PRO : process.env.PRICE_STARTER;
  if (!priceId) return res.status(400).json({ error: 'Invalid plan' });

  try {
    const session = await stripe.checkout.sessions.create({
      mode: 'subscription',
      payment_method_types: ['card'],
      line_items: [{ price: priceId, quantity: 1 }],
      success_url: 'https://glasik.mooo.com/success?session_id={CHECKOUT_SESSION_ID}',
      cancel_url: 'https://glasik.mooo.com/#pricing',
      metadata: {
        api_key_prefix: req.apiKey.key_prefix,
        api_key_hash: hashKey(req.headers['x-api-key']),
        plan,
      },
    });
    res.json({ checkout_url: session.url });
  } catch(e) {
    res.status(500).json({ error: e.message });
  }
});

// Compress
app.post('/compress', authMiddleware, async (req, res) => {
  try {
    const { data } = req.body;
    if (!data || typeof data !== 'string') {
      return res.status(400).json({ error: 'data (base64 string) required' });
    }
    const buf = Buffer.from(data, 'base64');
    if (buf.length === 0) return res.status(400).json({ error: 'Empty data' });
    if (buf.length > 10 * 1024 * 1024) return res.status(413).json({ error: 'Max 10MB per request' });

    const result = await encoder.compress(buf);
    const mb = buf.length / (1024 * 1024);
    trackUsage(req.apiKey.key_prefix, mb, '/compress');

    // Report to Stripe meter if paid plan
    if (req.apiKey.plan !== 'free' && req.apiKey.plan !== 'trial' && req.apiKey.customer_id) {
      stripe.billing.meterEvents.create({
        event_name: process.env.METER_EVENT,
        payload: {
          stripe_customer_id: req.apiKey.customer_id,
          value: Math.ceil(mb * 10).toString(),
        },
      }).catch(e => console.error('meter error:', e.message));
    }

    res.json({
      success: true,
      original_bytes: buf.length,
      compressed_bytes: result.compressed.length,
      compressed: result.compressed.toString('base64'),
      ratio: parseFloat(result.ratio.toFixed(4)),
      mode: result.mode,
    });
  } catch(e) {
    res.status(500).json({ error: e.message });
  }
});

// Decompress
app.post('/decompress', authMiddleware, async (req, res) => {
  try {
    const { data } = req.body;
    if (!data || typeof data !== 'string') {
      return res.status(400).json({ error: 'data (base64 string) required' });
    }
    const compressed = Buffer.from(data, 'base64');
    const decompressed = await encoder.decompress(compressed);
    if (!decompressed) throw new Error('decompression failed');

    const mb = compressed.length / (1024 * 1024);
    trackUsage(req.apiKey.key_prefix, mb, '/decompress');

    res.json({
      success: true,
      compressed_bytes: compressed.length,
      original_bytes: decompressed.length,
      data: decompressed.toString('base64'),
    });
  } catch(e) {
    res.status(500).json({ error: e.message });
  }
});

// Stats
app.get('/stats', authMiddleware, (req, res) => {
  const usage = db.prepare(`
    SELECT SUM(mb_compressed) as total_mb, COUNT(*) as requests
    FROM usage_log WHERE key_prefix = ?
  `).get(req.apiKey.key_prefix);

  res.json({
    plan: req.apiKey.plan,
    mb_used_this_month: parseFloat((req.apiKey.mb_used_this_month || 0).toFixed(4)),
    mb_limit: req.apiKey.mb_limit,
    total_mb_all_time: parseFloat((usage.total_mb || 0).toFixed(4)),
    total_requests: usage.requests,
    trial_ends: req.apiKey.trial_ends_at || null,
    member_since: req.apiKey.created_at,
  });
});

// Success page
app.get('/success', (req, res) => {
  res.send(`<!DOCTYPE html><html><head><title>Glasik — Upgraded</title>
  <style>body{background:#020408;color:#e2e8f0;font-family:monospace;display:flex;align-items:center;justify-content:center;height:100vh;text-align:center}
  h1{color:#00ff88;font-size:48px;margin-bottom:16px} p{color:#4b5563}</style></head>
  <body><div><h1>🔮</h1><h1>You're upgraded.</h1><p>Your API key is now active on your new plan.</p>
  <p style="margin-top:24px"><a href="https://glasik.mooo.com" style="color:#00ff88">← Back to Glasik</a></p>
  </div></body></html>`);
});

// Reset monthly usage (run via cron)
app.post('/admin/reset-monthly', (req, res) => {
  const secret = req.headers['x-admin-secret'];
  if (secret !== process.env.ADMIN_SECRET) return res.status(401).json({ error: 'unauthorized' });
  db.prepare('UPDATE api_keys SET mb_used_this_month = 0').run();
  res.json({ success: true, message: 'Monthly usage reset' });
});

app.listen(PORT, () => {
  console.log(`GN API v2 running on port ${PORT}`);
});



// /compress-batch — V4 round-trip codec with GCdict
app.post('/compress-batch', authMiddleware, async (req, res) => {
  try {
    const { messages, dict_size = 400 } = req.body;
    if (!Array.isArray(messages) || messages.length === 0)
      return res.status(400).json({ error: 'messages array required' });
    if (messages.length > 1000) return res.status(400).json({ error: 'Max 1000 messages' });
    const zlib = require('zlib');
    const bufs = messages.map(m => Buffer.from(m, 'base64'));
    const totalOrig = bufs.reduce((s,b) => s+b.length, 0);
    if (totalOrig > 10*1024*1024) return res.status(413).json({ error: 'Max 10MB' });
    const trainN = Math.min(dict_size, bufs.length);
    const litStreams = [];
    for (let i = 0; i < trainN; i++) {
      const r = await gn.gnSplitRaw([bufs[i]]);
      if (r[1] && r[1].length > 0) litStreams.push(Buffer.from(r[1]));
    }
    const gcdict = Buffer.concat(litStreams).slice(-32768);
    const d6 = (b, dict) => { try { return zlib.deflateRawSync(b,{level:6,dictionary:dict}); } catch(e) { return zlib.deflateRawSync(b,{level:6}); }};
    let totalCompressed = 0;
    const encoded = [];
    for (const buf of bufs) {
      const [toks, lits, runs] = await gn.gnSplitRawV4(buf);
      const cToks = d6(Buffer.from(toks));
      const cLits = d6(Buffer.from(lits), gcdict);
      const cRuns = d6(Buffer.from(runs));
      totalCompressed += cToks.length + cLits.length + cRuns.length;
      encoded.push({ toks: cToks.toString('base64'), lits: cLits.toString('base64'), runs: cRuns.toString('base64') });
    }
    trackUsage(req.apiKey.key_prefix, totalOrig/(1024*1024), '/compress-batch');
    res.json({ success: true, gcdict: gcdict.toString('base64'), messages: encoded, original_bytes: totalOrig, compressed_bytes: totalCompressed, ratio: parseFloat((totalOrig/totalCompressed).toFixed(4)), count: bufs.length, mode: 'gn_v4_batch' });
  } catch(e) { res.status(500).json({ error: e.message }); }
});

// /decompress-batch — reconstruct originals from V4 streams + gcdict
app.post('/decompress-batch', authMiddleware, async (req, res) => {
  try {
    const { gcdict, messages } = req.body;
    if (!gcdict || !Array.isArray(messages) || messages.length === 0)
      return res.status(400).json({ error: 'gcdict and messages required' });
    const zlib = require('zlib');
    const dict = Buffer.from(gcdict, 'base64');
    const inf = (b, d) => { try { return zlib.inflateRawSync(b,{dictionary:d}); } catch(e) { return zlib.inflateRawSync(b); }};
    const results = [];
    for (const m of messages) {
      const toks = zlib.inflateRawSync(Buffer.from(m.toks,'base64'));
      const lits = inf(Buffer.from(m.lits,'base64'), dict);
      const runs = zlib.inflateRawSync(Buffer.from(m.runs,'base64'));
      const restored = await gn.gnMergeRawV4(toks, lits, runs);
      results.push(Buffer.from(restored).toString('base64'));
    }
    res.json({ success: true, messages: results, count: results.length });
  } catch(e) { res.status(500).json({ error: e.message }); }
});
