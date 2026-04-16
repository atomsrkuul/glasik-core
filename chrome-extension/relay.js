// ISOLATED world -- can use chrome APIs, listens to postMessage from MAIN world
window.addEventListener('message', e => {
  if (e.source !== window || !e.data?.__GN__) return;
  const { role, content, sessionId } = e.data;
  console.log('[GN relay] storing', role, content.length + 'B', sessionId);
  chrome.runtime.sendMessage({ type: 'GN_STORE', role, content, sessionId });
});
console.log('[GN] Relay active');
