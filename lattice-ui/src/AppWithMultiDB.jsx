/**
 * Multi-Database Lattice Viewer
 * Tabs for each shard database: Glasik, Claude, OpenClaw
 * Each shows real-time lattice visualization from respective DB
 */

import { useEffect, useState } from "react";
import App from "./App";

const DATABASES = [
  {
    id: 'glasik',
    name: 'Glasik Memory',
    color: '#7c3aed',
    csvPath: 'data/glasik-metrics.csv',
    description: 'My memories and identity (45 shards)'
  },
  {
    id: 'claude',
    name: 'Claude LLM',
    color: '#3b82f6',
    csvPath: 'data/claude-metrics.csv',
    description: 'Claude API conversations'
  },
  {
    id: 'openclaw',
    name: 'OpenClaw Agent',
    color: '#10b981',
    csvPath: '~/.openclaw/gn-metrics.csv',
    description: 'Real-time agent traffic (3570+ messages)'
  }
];

export default function AppWithMultiDB() {
  const [activeTab, setActiveTab] = useState('glasik');
  const [dbStatus, setDbStatus] = useState({});
  const [shardCounts, setShardCounts] = useState({});

  useEffect(() => {
    // Check which databases have metrics available
    const checkDatabases = async () => {
      const status = {};
      const counts = {};

      for (const db of DATABASES) {
        try {
          // Try to fetch metrics from lattice API
          const response = await fetch(`/api/gn-metrics/${db.id}`, { signal: AbortSignal.timeout(2000) });
          if (response.ok) {
            const data = await response.json();
            status[db.id] = 'active';
            counts[db.id] = data.shards || 0;
          } else {
            status[db.id] = 'inactive';
            counts[db.id] = 0;
          }
        } catch (err) {
          status[db.id] = 'inactive';
          counts[db.id] = 0;
        }
      }

      setDbStatus(status);
      setShardCounts(counts);
    };

    checkDatabases();
    const interval = setInterval(checkDatabases, 10000); // Check every 10s
    return () => clearInterval(interval);
  }, []);

  const activeDb = DATABASES.find(d => d.id === activeTab);
  const isActive = dbStatus[activeTab] === 'active';

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', backgroundColor: '#0f172a' }}>
      {/* Tabs */}
      <div style={{
        display: 'flex',
        borderBottom: '2px solid #1e293b',
        backgroundColor: '#020617',
        padding: '0 1rem'
      }}>
        {DATABASES.map(db => (
          <button
            key={db.id}
            onClick={() => setActiveTab(db.id)}
            style={{
              padding: '1rem 1.5rem',
              border: 'none',
              background: activeTab === db.id
                ? `linear-gradient(135deg, ${db.color}22, ${db.color}11)`
                : 'transparent',
              color: activeTab === db.id ? db.color : '#64748b',
              borderBottom: activeTab === db.id ? `3px solid ${db.color}` : 'none',
              cursor: 'pointer',
              fontSize: '0.95rem',
              fontWeight: activeTab === db.id ? '600' : '400',
              transition: 'all 0.2s',
              position: 'relative'
            }}
            onMouseEnter={(e) => {
              if (activeTab !== db.id) {
                e.target.style.color = '#cbd5e1';
              }
            }}
            onMouseLeave={(e) => {
              if (activeTab !== db.id) {
                e.target.style.color = '#64748b';
              }
            }}
          >
            <span>{db.name}</span>
            {isActive && activeTab === db.id && (
              <span style={{
                display: 'inline-block',
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                backgroundColor: db.color,
                marginLeft: '0.75rem',
                animation: 'pulse 2s infinite'
              }} />
            )}
            {shardCounts[db.id] > 0 && (
              <span style={{
                display: 'inline-block',
                fontSize: '0.75rem',
                backgroundColor: db.color + '33',
                color: db.color,
                padding: '0.25rem 0.5rem',
                borderRadius: '4px',
                marginLeft: '0.75rem'
              }}>
                {shardCounts[db.id]} shards
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Status bar */}
      <div style={{
        padding: '0.75rem 1.5rem',
        backgroundColor: '#1e293b',
        borderBottom: '1px solid #334155',
        fontSize: '0.85rem',
        color: '#94a3b8',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center'
      }}>
        <div>
          <strong style={{ color: activeDb.color }}>{activeDb.name}</strong>
          <span style={{ marginLeft: '1rem' }}>{activeDb.description}</span>
        </div>
        <div>
          {isActive ? (
            <span style={{ color: activeDb.color }}>● LIVE</span>
          ) : (
            <span style={{ color: '#ef4444' }}>● No data</span>
          )}
        </div>
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflow: 'auto', position: 'relative' }}>
        {isActive ? (
          <App 
            dataSource={activeDb.csvPath}
            databaseId={activeTab}
            accentColor={activeDb.color}
          />
        ) : (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            height: '100%',
            flexDirection: 'column',
            color: '#64748b'
          }}>
            <div style={{ fontSize: '3rem', marginBottom: '1rem' }}>🔇</div>
            <div>No metrics available for {activeDb.name}</div>
            <div style={{ fontSize: '0.85rem', marginTop: '0.5rem' }}>
              Waiting for data...
            </div>
          </div>
        )}
      </div>

      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }
      `}</style>
    </div>
  );
}
