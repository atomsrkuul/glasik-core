# GN Chrome Extension

Middleware for capturing Claude.ai conversations through GN compression.

## Install
1. Open Chrome → chrome://extensions/
2. Enable Developer mode
3. Click "Load unpacked" → select this folder
4. Visit https://claude.ai and conversations are automatically stored via GN

## Requirements
- GN native host registered: `gn-native-host.js` + `com.glasik.gn_compression.json`
- Native host manifest installed in Chrome/Chromium NativeMessagingHosts

## Auto-install native host
```bash
node install-native-host.js
```
