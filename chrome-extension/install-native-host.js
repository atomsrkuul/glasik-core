#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const os = require('os');

const hostScript = path.join(__dirname, '..', '..', '.openclaw', 'workspace', 'src', 'gn-native-host.js');
const manifest = {
  name: 'com.glasik.gn_compression',
  description: 'GN Compression Native Host',
  path: hostScript,
  type: 'stdio',
  allowed_origins: ['chrome-extension://llnhjgamebhfelehailfofpcfbjnjgdb/']
};

const dirs = [
  path.join(os.homedir(), '.config', 'chromium', 'NativeMessagingHosts'),
  path.join(os.homedir(), '.config', 'google-chrome', 'NativeMessagingHosts'),
];

dirs.forEach(dir => {
  fs.mkdirSync(dir, { recursive: true });
  const dest = path.join(dir, 'com.glasik.gn_compression.json');
  fs.writeFileSync(dest, JSON.stringify(manifest, null, 2));
  console.log('installed:', dest);
});
console.log('Native host registered. Load the extension in Chrome.');
