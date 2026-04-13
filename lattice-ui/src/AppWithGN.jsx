/**
 * Lattice UI with optional GN compression data
 * 
 * This is a wrapper around App.jsx that:
 * 1. Keeps lattice-ui working exactly as before
 * 2. Optionally loads real GN compression metrics
 * 3. Visualizes them as shards in the crystal lattice
 * 4. Falls back to demo data if GN not available
 * 
 * Non-breaking: If GN is unavailable, falls back to existing behavior.
 */

import { useState, useEffect } from 'react';
import App from './App';
import { useGNMetrics, generateDemoGNGraph } from './gn-bridge';

export default function AppWithGN({ useGNData = true, enableDemoData = false }) {
  const [shards, setShards] = useState({});
  const [graph, setGraph] = useState({});
  const [dataSource, setDataSource] = useState('none');

  // Try to load real GN metrics
  const gnData = useGNMetrics('/api/gn-metrics/stats');

  useEffect(() => {
    if (!useGNData) {
      // Use existing behavior
      setDataSource('default');
      return;
    }

    // Real GN data available?
    if (gnData.shards && Object.keys(gnData.shards).length > 0) {
      setShards(gnData.shards);
      setGraph(gnData.graph);
      setDataSource('gn-real');
      console.log('[Lattice] Loading real GN compression data');
    } 
    // Fall back to demo data?
    else if (enableDemoData) {
      const demoGraph = generateDemoGNGraph(30);
      setShards(demoGraph.shards);
      setGraph(demoGraph.graph);
      setDataSource('gn-demo');
      console.log('[Lattice] Using demo GN compression data');
    }
    // No data available
    else {
      setDataSource('default');
    }
  }, [gnData, useGNData, enableDemoData]);

  // Merge GN data with existing props
  const appProps = {
    // ... any existing props would go here
    // If data is from GN, pass it to App
    ...(dataSource !== 'default' ? { initialShards: shards, initialGraph: graph } : {}),
  };

  return (
    <div style={{ position: 'relative' }}>
      <App {...appProps} />
      
      {/* Optional: status indicator */}
      <div style={{
        position: 'absolute',
        top: 10,
        right: 10,
        fontSize: '12px',
        color: '#aaa',
        background: 'rgba(0,0,0,0.5)',
        padding: '8px 12px',
        borderRadius: '4px',
        fontFamily: 'monospace',
      }}>
        {dataSource === 'gn-real' && '✓ GN Real Data'}
        {dataSource === 'gn-demo' && '◇ GN Demo Data'}
        {dataSource === 'default' && 'Default Mode'}
        {gnData.loading && ' (loading...)'}
      </div>
    </div>
  );
}
