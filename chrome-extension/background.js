// GN Compression background - native messaging bridge
const HOST = 'com.glasik.gn_compression';
let port = null;

function connectNative() {
  try {
    port = chrome.runtime.connectNative(HOST);
    port.onMessage.addListener(msg => {
      console.log('[GN] native response:', msg);
      if (msg.type === 'stored') {
        chrome.action.setBadgeText({ text: '●' });
        chrome.action.setBadgeBackgroundColor({ color: '#34d399' });
      }
    });
    port.onDisconnect.addListener(() => {
      console.log('[GN] native host disconnected:', chrome.runtime.lastError?.message);
      port = null;
      setTimeout(connectNative, 3000);
    });
    // Ping to verify
    port.postMessage({ type: 'ping' });
    console.log('[GN] native host connected');
  } catch(e) {
    console.error('[GN] native connect failed:', e);
  }
}

// Listen for messages from content script
chrome.runtime.onMessage.addListener((msg, sender, reply) => {
  if (msg.type === 'get_stats') {
    fetch('http://localhost:8888/api/metrics')
      .then(r => r.json())
      .then(d => reply({ shards: d.total || 0, ratio: d.ratioStats?.avg_ratio?.toFixed(2) || 0, mode: 'active' }))
      .catch(() => reply({ shards: 0, ratio: 0, mode: 'inactive' }));
    return true;
  }
});

// Listen for store requests from content script via postMessage relay
chrome.runtime.onMessageExternal || null;

// Content scripts can't directly use native messaging
// Use chrome.tabs to relay messages
chrome.runtime.onMessage.addListener((msg, sender) => {
  if (msg.type === 'GN_STORE' && port) {
    port.postMessage({
      type: 'store',
      role: msg.role,
      content: msg.content,
      sessionId: msg.sessionId,
    });
  }
});

connectNative();
