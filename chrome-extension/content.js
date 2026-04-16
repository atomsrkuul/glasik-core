const TARGETS = ['api.anthropic.com', 'a-api.anthropic.com'];
const originalFetch = window.fetch.bind(window);

function store(role, content, sessionId) {
  chrome.runtime.sendMessage({ type: 'GN_STORE', role, content, sessionId });
}

window.fetch = async function(url, options = {}) {
  const urlStr = typeof url === 'string' ? url : url?.url || '';
  const matched = TARGETS.find(t => urlStr.includes(t));
  
  if (matched) {
    let messages = [];
    let sessionId = 'gn-claude-' + Date.now().toString(36);
    try {
      const parsed = JSON.parse(options.body);
      messages = parsed.messages || [];
      const first = messages.find(m => m.role === 'user');
      if (first) {
        const text = typeof first.content === 'string' ? first.content : JSON.stringify(first.content);
        sessionId = 'gn-claude-' + btoa(text.slice(0, 24)).replace(/[^a-z0-9]/gi,'').slice(0,10);
        messages.forEach(m => {
          const c = typeof m.content === 'string' ? m.content
            : Array.isArray(m.content) ? m.content.filter(b=>b.type==='text').map(b=>b.text).join('\n')
            : JSON.stringify(m.content);
          if (c.length > 50) store(m.role, c, sessionId);
        });
      }
    } catch(e) {}

    const response = await originalFetch(url, options);
    response.clone().text().then(text => {
      let out = '';
      try {
        const d = JSON.parse(text);
        out = d.content ? d.content.filter(b=>b.type==='text').map(b=>b.text).join('\n') : '';
      } catch(e) {
        out = text.split('\n')
          .filter(l=>l.startsWith('data:')&&!l.includes('[DONE]'))
          .map(l=>{ try{return JSON.parse(l.slice(5));}catch{return null;} })
          .filter(Boolean).map(d=>d.delta?.text||d.completion||'').join('');
      }
      if (out.length > 50) store('assistant', out, sessionId);
    }).catch(()=>{});
    return response;
  }
  return originalFetch(url, options);
};

console.log('[GN] Native messaging middleware active');
