import { useState } from "react";
import App from "./App.jsx";
import AppLattice from "./AppLattice.jsx";

export default function AppWithTabs() {
  const [activeTab, setActiveTab] = useState('tab1');

  if (activeTab === 'tab2') {
    return (
      <div style={{ width: "100vw", height: "100vh", display: "flex", flexDirection: "column" }}>
        <div style={{
          display: "flex",
          height: "30px",
          background: "rgba(0,0,0,0.9)",
          borderBottom: "1px solid rgba(0,255,136,0.3)",
          paddingLeft: "10px",
          alignItems: "center",
          gap: "2px",
          zIndex: 1000,
        }}>
          <button onClick={() => setActiveTab('tab1')} style={{
            background: 'transparent',
            color: '#0f0',
            border: 'none',
            padding: '6px 12px',
            fontFamily: 'monospace',
            fontSize: '11px',
            cursor: 'pointer',
            borderBottom: '1px solid rgba(0,255,136,0.2)',
          }}>Lattice</button>
          <button onClick={() => setActiveTab('tab2')} style={{
            background: 'rgba(0,255,136,0.2)',
            color: '#0f0',
            border: 'none',
            padding: '6px 12px',
            fontFamily: 'monospace',
            fontSize: '11px',
            cursor: 'pointer',
            borderBottom: '2px solid #0f0',
          }}>Tab 2</button>
        </div>
        <div style={{ flex: 1, overflow: "hidden" }}>
          <AppLattice latticeFile="/lattice-openclaw.json" />
        </div>
      </div>
    );
  }

  return (
    <div style={{ width: "100vw", height: "100vh", display: "flex", flexDirection: "column" }}>
      <div style={{
        display: "flex",
        height: "30px",
        background: "rgba(0,0,0,0.9)",
        borderBottom: "1px solid rgba(0,255,136,0.3)",
        paddingLeft: "10px",
        alignItems: "center",
        gap: "2px",
        zIndex: 1000,
      }}>
        <button onClick={() => setActiveTab('tab1')} style={{
          background: 'rgba(0,255,136,0.2)',
          color: '#0f0',
          border: 'none',
          padding: '6px 12px',
          fontFamily: 'monospace',
          fontSize: '11px',
          cursor: 'pointer',
          borderBottom: '2px solid #0f0',
        }}>Lattice</button>
        <button onClick={() => setActiveTab('tab2')} style={{
          background: 'transparent',
          color: '#0f0',
          border: 'none',
          padding: '6px 12px',
          fontFamily: 'monospace',
          fontSize: '11px',
          cursor: 'pointer',
          borderBottom: '1px solid rgba(0,255,136,0.2)',
        }}>Tab 2</button>
      </div>
      <div style={{ flex: 1, overflow: "hidden" }}>
        <App />
      </div>
    </div>
  );
}
