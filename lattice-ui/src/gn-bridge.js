/**
 * GN to Lattice Bridge
 * 
 * Converts real GN compression metrics into lattice-ui visualization format.
 * Non-blocking: runs in parallel, doesn't affect lattice rendering.
 */

import { useEffect, useState } from 'react';

/**
 * Generate VTC-like identifier from compression metric
 * (mockup until real VTC is available from GN)
 */
export function metricToVTC(metric, index) {
  const id = [
    metric.session_id.substring(0, 8),
    metric.timestamp.substring(5, 10),
    (metric.original_size % 256).toString(16).padStart(2, '0'),
    (metric.compressed_size % 256).toString(16).padStart(2, '0'),
    (metric.compression_ratio * 100 | 0).toString(16).padStart(2, '0'),
  ].join('').substring(0, 32);
  
  return `VTC-v1-${id.padEnd(32, '0')}`;
}

/**
 * Convert GN metrics CSV to lattice graph format
 */
export function metricsToGraph(csvText) {
  if (!csvText) return { shards: {}, edges: {} };

  const lines = csvText.trim().split('\n');
  const headers = lines[0]?.split(',') || [];
  const graph = {};
  const shards = {};
  
  // Build graph from metric sequence
  let prevVTC = null;
  
  for (let i = 1; i < lines.length; i++) {
    const parts = lines[i].split(',');
    if (parts.length < headers.length) continue;

    const metric = {};
    headers.forEach((h, idx) => {
      metric[h.trim().replace(/^"|"$/g, '')] = parts[idx]?.trim().replace(/^"|"$/g, '');
    });

    const vtc = metricToVTC(metric, i);
    const type = metric.message_type || 'user_intent';
    const ratio = parseFloat(metric.compression_ratio) || 0;

    shards[vtc] = {
      vtc,
      type,
      count: ratio,
      originalSize: parseInt(metric.original_size) || 0,
      compressedSize: parseInt(metric.compressed_size) || 0,
      ratio: ratio.toFixed(2),
      latency: parseFloat(metric.latency_ms) || 0,
      cost: parseFloat(metric.cost_saved_usd) || 0,
      timestamp: metric.timestamp,
    };

    // Build edges (flow between messages)
    if (prevVTC) {
      graph[prevVTC] = graph[prevVTC] || {
        type,
        next: {},
      };
      graph[prevVTC].next[vtc] = (graph[prevVTC].next[vtc] || 0) + 1;
    }

    prevVTC = vtc;
  }

  return { shards, graph };
}

/**
 * Hook: Fetch GN metrics and update visualization
 */
export function useGNMetrics(metricsUrl = '/api/gn-metrics/stats') {
  const [shards, setShards] = useState({});
  const [graph, setGraph] = useState({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    const fetchMetrics = async () => {
      setLoading(true);
      try {
        // Try to fetch from GN metrics endpoint
        const response = await fetch(metricsUrl);
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        
        const data = await response.json();
        
        // Convert metrics to graph format
        const csvText = data.metricsCSV || '';
        const { shards: newShards, graph: newGraph } = metricsToGraph(csvText);
        
        setShards(newShards);
        setGraph(newGraph);
        setError(null);
      } catch (e) {
        console.log('[GN Bridge] Metrics not available yet (ok for now):', e.message);
        setError(e);
      } finally {
        setLoading(false);
      }
    };

    // Fetch on mount
    fetchMetrics();

    // Poll every 30 seconds for new metrics
    const interval = setInterval(fetchMetrics, 30000);
    return () => clearInterval(interval);
  }, [metricsUrl]);

  return { shards, graph, loading, error };
}

/**
 * Demo: Generate synthetic GN-like shard graph for testing
 */
export function generateDemoGNGraph(count = 20) {
  const graph = {};
  const shards = {};
  const types = ['user_intent', 'assistant_response', 'system_message'];
  
  let prevVTC = null;
  for (let i = 0; i < count; i++) {
    const type = types[i % types.length];
    const ratio = 2.4 + Math.random() * 0.2; // 2.4-2.6x typical for GN
    const vtc = `VTC-v1-${Math.random().toString(16).substring(2, 34)}`;

    shards[vtc] = {
      vtc,
      type,
      count: ratio,
      originalSize: 256 + Math.random() * 2048,
      compressedSize: (256 + Math.random() * 2048) / ratio,
      ratio: ratio.toFixed(2),
      latency: 0.02 + Math.random() * 0.03, // 0.02-0.05ms
      cost: Math.random() * 0.001,
      timestamp: new Date(Date.now() - i * 60000).toISOString(),
    };

    if (prevVTC) {
      graph[prevVTC] = graph[prevVTC] || { type, next: {} };
      graph[prevVTC].next[vtc] = 1 + Math.floor(Math.random() * 3);
    }

    prevVTC = vtc;
  }

  return { shards, graph };
}

export default {
  metricToVTC,
  metricsToGraph,
  useGNMetrics,
  generateDemoGNGraph,
};
