chrome.runtime.sendMessage({ type: 'get_stats' }, stats => {
  const statusEl = document.getElementById('status');
  document.getElementById('shards').textContent = stats.shards;
  document.getElementById('ratio').textContent = stats.ratio ? stats.ratio + 'x' : '-';
  if (stats.mode === 'active') {
    statusEl.textContent = '● Active';
    statusEl.className = 'status active';
  } else {
    statusEl.textContent = '○ Proxy offline';
    statusEl.className = 'status inactive';
  }
});
