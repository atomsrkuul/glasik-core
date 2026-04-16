function extractAndStore() {
  const path = location.pathname;
  const sessionId = 'gn-claude-' + (path.split('/').pop() || 'x').slice(0, 8);
  
  // User messages -- confirmed working testid
  document.querySelectorAll('[data-testid="user-message"]').forEach(el => {
    if (el.__gnDone) return;
    el.__gnDone = true;
    const content = el.innerText?.trim() || '';
    if (content.length > 10) {
      window.postMessage({ __GN__: true, role: 'user', content, sessionId }, '*');
    }
  });

  // Assistant messages -- find by structure (no testid, but follows user-message)
  // Claude renders assistant text in specific prose containers
  document.querySelectorAll('.prose, [class*="prose"]').forEach(el => {
    if (el.__gnDone || el.closest('[data-testid="user-message"]')) return;
    const content = el.innerText?.trim() || '';
    if (content.length > 100) {
      el.__gnDone = true;
      window.postMessage({ __GN__: true, role: 'assistant', content, sessionId }, '*');
    }
  });
}

function start() {
  extractAndStore();
  new MutationObserver(extractAndStore)
    .observe(document.body, { childList: true, subtree: true });
  console.log('[GN] extractor active');
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', start);
} else {
  start();
}
