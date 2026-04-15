/**
 * AppWithShards - Gold build (menu-sliders-working) + shard database tabs
 * Loads Glasik, GN, and future OpenClaw shard data
 * Does NOT modify original App.jsx
 */

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { ConvexGeometry } from "three/examples/jsm/geometries/ConvexGeometry.js";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { EffectComposer } from "three/examples/jsm/postprocessing/EffectComposer.js";
import { RenderPass } from "three/examples/jsm/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";

function vtcToCrystal(vtc) {
  const hex = vtc.replace("VTC-v1-", "");
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) bytes.push(parseInt(hex.substr(i, 2), 16));
  const n = bytes.length;
  const points = [];
  const height = 8 + (bytes[0] % 8);
  points.push(new THREE.Vector3(0, height, 0));

  const upperCount = 4 + (bytes[1] % 3);
  for (let i = 0; i < upperCount; i++) {
    const b = bytes[(2 + i) % n];
    const angle = (i / upperCount) * Math.PI * 2 + (b / 255) * 0.8;
    const radius = 3 + (b % 5);
    const y = 1 + (bytes[(3 + i) % n] % 4);
    points.push(new THREE.Vector3(Math.cos(angle) * radius, y, Math.sin(angle) * radius));
  }

  const lowerCount = upperCount + (bytes[9 % n] % 2);
  for (let i = 0; i < lowerCount; i++) {
    const b = bytes[(10 + i) % n];
    const angle = (i / lowerCount) * Math.PI * 2 + (b / 255) * 0.6;
    const radius = 4 + (b % 6);
    const y = -(1 + (bytes[(12 + i) % n] % 3));
    points.push(new THREE.Vector3(Math.cos(angle) * radius, y, Math.sin(angle) * radius));
  }

  const depth = -(5 + (bytes[n - 1] % 6));
  points.push(new THREE.Vector3(0, depth, 0));
  return { points, upperCount, lowerCount };
}

function buildCrystalGeometry(vtc) {
  const { points, upperCount, lowerCount } = vtcToCrystal(vtc);
  const geo = new THREE.BufferGeometry();
  const verts = [];
  const norms = [];

  const apex = points[0];
  const upper = points.slice(1, 1 + upperCount);
  const lower = points.slice(1 + upperCount, 1 + upperCount + lowerCount);
  const base = points[points.length - 1];

  function addTri(a, b, c) {
    const n = new THREE.Vector3().crossVectors(
      new THREE.Vector3().subVectors(b, a),
      new THREE.Vector3().subVectors(c, a)
    ).normalize();
    [a, b, c].forEach(p => { verts.push(p.x, p.y, p.z); norms.push(n.x, n.y, n.z); });
  }

  for (let i = 0; i < upperCount; i++) addTri(apex, upper[i], upper[(i + 1) % upperCount]);
  for (let i = 0; i < upperCount; i++) {
    const u0 = upper[i], u1 = upper[(i + 1) % upperCount];
    const l0 = lower[i % lowerCount], l1 = lower[(i + 1) % lowerCount];
    addTri(u0, l0, u1);
    addTri(u1, l0, l1);
  }
  for (let i = 0; i < lowerCount; i++) addTri(base, lower[(i + 1) % lowerCount], lower[i]);

  geo.setAttribute("position", new THREE.Float32BufferAttribute(verts, 3));
  geo.setAttribute("normal", new THREE.Float32BufferAttribute(norms, 3));
  return geo;
}

// Dynamic shard sources from dashboard API
// Falls back to static files if API unavailable
const DASHBOARD_API = 'http://localhost:8888';
const SHARD_SOURCES = [
  { id: "sess-e2e", label: "E2E Test", file: DASHBOARD_API+"/api/lattice/sess-e2e" },
  { id: "sess1", label: "Session 1", file: DASHBOARD_API+"/api/lattice/sess1" },
  { id: "glasik", label: "Glasik", file: "/lattice-glasik.json" },
  { id: "gn", label: "GN Static", file: "/lattice-gn.json" },
];

// Auto-discover namespaces from dashboard
async function fetchNamespaces() {
  try {
    const r = await fetch(DASHBOARD_API+'/api/namespaces');
    if (!r.ok) return SHARD_SOURCES;
    const sessions = await r.json();
    return sessions.map(s => ({
      id: s.session_id,
      label: s.session_id.slice(0,20)+' ('+s.cnt+')',
      file: DASHBOARD_API+'/api/lattice/'+s.session_id,
      avgRatio: s.avg_ratio,
    }));
  } catch(e) {
    return SHARD_SOURCES;
  }
}

const TYPE_COLOR = {
  user_intent:        0x00ff88,
  assistant_response: 0x2288ff,
  system_message:     0xff8800,
  code_block:         0xffee00,
  tool_call:          0xdd00ff,
  tool_result:        0x00eeff,
  batch:              0x44ff44,
  user:               0x00ff88,
  assistant:          0x2288ff,
};

function VisualizerCanvas({ dbFile, dbLabel }) {
  const ref = useRef(null);
  const [shardCount, setShardCount] = useState(0);

  useEffect(() => {
    if (!ref.current) return;

    const W = window.innerWidth - 80, H = window.innerHeight - 60;
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x010408);

    const camera = new THREE.PerspectiveCamera(60, W / H, 0.1, 5000);
    camera.position.set(0, 30, 280);

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setClearColor(0x010408);

    ref.current.innerHTML = "";
    ref.current.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(0, -5, 0);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 50;
    controls.maxDistance = 600;
    controls.update();

    // bloom
    const composer = new EffectComposer(renderer);
    composer.addPass(new RenderPass(scene, camera));
    const bloom = new UnrealBloomPass(new THREE.Vector2(W, H), 0.7, 0.4, 0.82);
    composer.addPass(bloom);

    // lights
    scene.add(new THREE.AmbientLight(0x112233, 2));
    const dir = new THREE.DirectionalLight(0xffffff, 2.5);
    dir.position.set(80, 120, 60);
    scene.add(dir);
    const fill = new THREE.DirectionalLight(0x334466, 1);
    fill.position.set(-80, -40, -60);
    scene.add(fill);

    const meshes = {};

    fetch(dbFile)
      .then(r => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json();
      })
      .then(graph => {
        const keys = Object.keys(graph);
        const total = keys.length;
        setShardCount(total);

        keys.forEach((vtc, idx) => {
          const node = graph[vtc];
          const color = TYPE_COLOR[node.type] || 0x44ff44;

          const phi = Math.acos(1 - (2 * (idx + 0.5)) / total);
          const theta = Math.PI * (1 + Math.sqrt(5)) * idx;
          const R = 130;

          const ox = R * Math.sin(phi) * Math.cos(theta);
          const oy = R * Math.sin(phi) * Math.sin(theta) - 5;
          const oz = R * Math.cos(phi);

          const geo = buildCrystalGeometry(vtc);
          const mat = new THREE.MeshPhongMaterial({
            color,
            emissive: new THREE.Color(color).multiplyScalar(0.2),
            shininess: 220,
            specular: 0xffffff,
            wireframe: false,
          });
          const mesh = new THREE.Mesh(geo, mat);
          mesh.position.set(ox, oy, oz);
          const baseScale = 0.9 + Math.log2(node.count + 1) * 0.6;
          mesh.scale.setScalar(baseScale * 1.5);
          mesh.userData = { vtc, color, index: idx };
          scene.add(mesh);
          meshes[vtc] = mesh;
        });

        const animate = () => {
          requestAnimationFrame(animate);
          controls.update();
          Object.values(meshes).forEach(m => {
            m.rotation.x += 0.0005;
            m.rotation.y += 0.001;
          });
          composer.render();
        };

        animate();
      })
      .catch(err => {
        console.error(`[${dbLabel}]`, err);
      });

    return () => {
      if (ref.current?.children.length) {
        ref.current.innerHTML = "";
      }
    };
  }, [dbFile, dbLabel]);

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <div
        ref={ref}
        style={{
          width: "100%",
          height: "100%",
          background: "#010408",
        }}
      />
      {shardCount > 0 && (
        <div style={{
          position: "absolute",
          top: 10,
          right: 10,
          background: "rgba(0,0,0,0.8)",
          color: "#0f0",
          padding: "8px 12px",
          borderRadius: "4px",
          fontFamily: "monospace",
          fontSize: "12px",
        }}>
          {shardCount} shards
        </div>
      )}
    </div>
  );
}

export default function AppWithShards() {
  const [activeTab, setActiveTab] = useState("sess-e2e");
  const [sources, setSources] = useState(SHARD_SOURCES);
  
  useEffect(() => {
    fetchNamespaces().then(ns => {
      if (ns.length > 0) {
        setSources(ns);
        setActiveTab(ns[0].id);
      }
    });
    // Refresh every 30s
    const iv = setInterval(() => {
      fetchNamespaces().then(ns => { if (ns.length > 0) setSources(ns); });
    }, 30000);
    return () => clearInterval(iv);
  }, []);
  const activeDb = sources.find(db => db.id === activeTab) || sources[0];

  return (
    <div style={{ display: "flex", flexDirection: "column", width: "100vw", height: "100vh", background: "#010408" }}>
      {/* Tab bar */}
      <div style={{
        display: "flex",
        height: "40px",
        background: "#111",
        borderBottom: "2px solid #0f0",
        paddingLeft: "10px",
        gap: "0px",
        alignItems: "center",
      }}>
        {sources.map(db => (
          <button
            key={db.id}
            onClick={() => setActiveTab(db.id)}
            style={{
              background: activeTab === db.id ? "#0f0" : "#000",
              color: activeTab === db.id ? "#000" : "#0f0",
              border: "none",
              padding: "8px 20px",
              fontFamily: "monospace",
              fontSize: "12px",
              fontWeight: activeTab === db.id ? "bold" : "normal",
              cursor: "pointer",
              borderBottom: activeTab === db.id ? "3px solid #000" : "1px solid #333",
              transition: "all 0.15s",
            }}
          >
            {db.label}
          </button>
        ))}
        <div style={{ flex: 1 }} />
        <div style={{
          color: "#0f0",
          fontFamily: "monospace",
          fontSize: "11px",
          marginRight: "10px",
          opacity: 0.6,
        }}>
          {activeDb?.label}
        </div>
      </div>

      {/* Canvas area */}
      <div style={{ flex: 1, overflow: "hidden" }}>
        <VisualizerCanvas key={activeTab} dbFile={activeDb.file} dbLabel={activeDb.label} />
      </div>
    </div>
  );
}
