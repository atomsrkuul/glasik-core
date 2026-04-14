/**
 * AppLite - Lightweight lattice viewer with size slider
 * Loads lattice.json with timeout protection
 * Renders THREE.js canvas without heavy state management
 */

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { ConvexGeometry } from "three/examples/jsm/geometries/ConvexGeometry.js";

console.log("[AppLite] Component initializing...");

// Import TubeGeometry
const setupGeometries = () => {
  if (!THREE.TubeGeometry) {
    console.warn('[AppLite] TubeGeometry not available, using LineBasicMaterial fallback');
  }
};

setupGeometries();

const TYPE_COLOR = {
  user_intent: 0xff8800,
  user: 0xff8800,
  assistant_response: 0x44ddff,
  assistant: 0x44ddff,
  system_message: 0xff6600,
  code_block: 0xffee00,
  tool_call: 0xdd00ff,
  tool_result: 0xaa44ff,
  toolResult: 0xaa44ff,
  batch: 0x00ff00,
};

function vtcToCrystal(vtc) {
  const hex = vtc.replace("VTC-v1-", "");
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(parseInt(hex.substr(i, 2), 16));
  }
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
    points.push(
      new THREE.Vector3(Math.cos(angle) * radius, y, Math.sin(angle) * radius)
    );
  }

  const lowerCount = upperCount + (bytes[9 % n] % 2);
  for (let i = 0; i < lowerCount; i++) {
    const b = bytes[(10 + i) % n];
    const angle = (i / lowerCount) * Math.PI * 2 + (b / 255) * 0.6;
    const radius = 4 + (b % 6);
    const y = -(1 + (bytes[(12 + i) % n] % 3));
    points.push(
      new THREE.Vector3(Math.cos(angle) * radius, y, Math.sin(angle) * radius)
    );
  }

  const depth = -(5 + (bytes[n - 1] % 6));
  points.push(new THREE.Vector3(0, depth, 0));
  return { points, upperCount, lowerCount };
}

function buildCrystalFromPairs(pairs) {
  if (!pairs || pairs.length < 4) {
    return buildCrystalGeometry('VTC-v1-default');
  }

  const pointsVec = [];
  const freq = {};
  
  pairs.forEach(({ lit, tok }) => {
    freq[lit] = (freq[lit] || 0) + 1;
  });

  const total = pairs.length;
  let entropy = 0;
  Object.values(freq).forEach((c) => {
    const p = c / total;
    entropy -= p * Math.log2(p);
  });

  const heightScale = 0.6 + Math.min(1.5, entropy);
  const densityFactor = Math.min(3, pairs.length / 10);

  pairs.forEach(({ lit, tok }) => {
    for (let d = 0; d < densityFactor; d++) {
      const angle = (tok * 0.3) + (lit % 11) * 0.25 + Math.sin(tok * 0.7) * 0.5;
      const radius = Math.min(2.5, Math.log2(lit + 1));
      const skewX = 0.5 + (lit % 7) * 0.18;
      const skewY = 0.5 + (tok % 9) * 0.15;
      const skewZ = 0.3 + ((lit + tok) % 11) * 0.12;
      
      const x = (Math.sin(tok * 1.3) + Math.cos(lit * 0.7)) * radius * skewX;
      const y = (Math.cos(tok * 0.9) - Math.sin(lit * 1.1)) * radius * skewY;
      const z = (Math.sin(tok * 0.5 + lit * 0.3)) * radius * skewZ;
      
      pointsVec.push(new THREE.Vector3(x, y, z));
    }
  });

  if (pointsVec.length < 4) return buildCrystalGeometry('VTC-v1-default');

  try {
    const geo = new ConvexGeometry(pointsVec);
    geo.computeBoundingBox();
    const center = new THREE.Vector3();
    geo.boundingBox.getCenter(center);
    geo.translate(-center.x, -center.y, -center.z);
    
    geo.computeBoundingSphere();
    const radius = geo.boundingSphere.radius || 1;
    const scale = 4.5 / radius;
    geo.scale(scale, scale, scale);
    
    return geo;
  } catch (e) {
    return buildCrystalGeometry('VTC-v1-default');
  }
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
    const n = new THREE.Vector3()
      .crossVectors(
        new THREE.Vector3().subVectors(b, a),
        new THREE.Vector3().subVectors(c, a)
      )
      .normalize();

    [a, b, c].forEach((p) => {
      verts.push(p.x, p.y, p.z);
      norms.push(n.x, n.y, n.z);
    });
  }

  for (let i = 0; i < upperCount; i++) {
    addTri(apex, upper[i], upper[(i + 1) % upperCount]);
  }

  for (let i = 0; i < upperCount; i++) {
    const u0 = upper[i];
    const u1 = upper[(i + 1) % upperCount];
    const l0 = lower[i % lowerCount];
    const l1 = lower[(i + 1) % lowerCount];
    addTri(u0, l0, u1);
    addTri(u1, l0, l1);
  }

  for (let i = 0; i < lowerCount; i++) {
    addTri(base, lower[(i + 1) % lowerCount], lower[i]);
  }

  geo.setAttribute("position", new THREE.Float32BufferAttribute(verts, 3));
  geo.setAttribute("normal", new THREE.Float32BufferAttribute(norms, 3));
  return geo;
}

export default function AppLite() {
  const ref = useRef(null);
  const [shardScale, setShardScale] = useState(0.5);
  const meshesRef = useRef({});

  // Update shard scales when slider changes
  useEffect(() => {
    Object.entries(meshesRef.current).forEach(([vtc, data]) => {
      const { mesh, baseScale } = data;
      mesh.scale.setScalar(baseScale * shardScale);
    });
  }, [shardScale]);

  // Initialize scene
  useEffect(() => {
    console.log("[AppLite] useEffect starting...");

    if (!ref.current) {
      console.error("[AppLite] ❌ ref.current is null");
      return;
    }

    console.log("[AppLite] ✓ ref.current exists, initializing THREE.js...");

    const W = window.innerWidth;
    const H = window.innerHeight;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x010408);

    const camera = new THREE.PerspectiveCamera(60, W / H, 0.1, 5000);
    camera.position.set(0, 0, 300);

    const renderer = new THREE.WebGLRenderer({ antialias: true, precision: 'highp' });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.3;
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFShadowShadowMap;

    console.log("[AppLite] ✓ Renderer created, appending to DOM...");
    ref.current.innerHTML = "";
    ref.current.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(0, -5, 0);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 50;
    controls.maxDistance = 600;
    controls.update();

    scene.add(new THREE.AmbientLight(0x334455, 1.5));

    const dir = new THREE.DirectionalLight(0xffffff, 3);
    dir.position.set(100, 150, 80);
    dir.castShadow = true;
    scene.add(dir);

    const fill = new THREE.DirectionalLight(0x6688cc, 1.5);
    fill.position.set(-100, -50, -80);
    scene.add(fill);

    const rim = new THREE.DirectionalLight(0xff6600, 0.8);
    rim.position.set(0, 0, -150);
    scene.add(rim);

    console.log("[AppLite] ✓ Scene setup complete, fetching lattice.json...");

    const loadTimeout = setTimeout(() => {
      console.error("[AppLite] ❌ Lattice.json load timeout");
    }, 5000);

    fetch("/lattice.json")
      .then((r) => {
        console.log("[AppLite] ✓ lattice.json fetched, parsing...");
        return r.json();
      })
      .then((graph) => {
        clearTimeout(loadTimeout);
        console.log(`[AppLite] ✓ Graph loaded, ${Object.keys(graph).length} shards`);

        const keys = Object.keys(graph);
        const total = keys.length;

        const edges = [];
        const shardMap = {};
        
        keys.forEach((vtc, idx) => {
          shardMap[vtc] = idx;
          const node = graph[vtc];
          const color = TYPE_COLOR[node.type] || 0x44ff44;

          const phi = Math.acos(1 - (2 * (idx + 0.5)) / total);
          const theta = Math.PI * (1 + Math.sqrt(5)) * idx;
          const R = 130;

          const ox = R * Math.sin(phi) * Math.cos(theta);
          const oy = R * Math.sin(phi) * Math.sin(theta) - 5;
          const oz = R * Math.cos(phi);

          const geo = buildCrystalFromPairs(node.pairs) || buildCrystalGeometry(vtc);
          const mat = new THREE.MeshStandardMaterial({
            color,
            emissive: new THREE.Color(color).multiplyScalar(0.15),
            metalness: 0.6,
            roughness: 0.2,
            flatShading: true,
            side: THREE.DoubleSide,
          });

          const mesh = new THREE.Mesh(geo, mat.clone());
          mesh.position.set(ox, oy, oz);

          const baseScale = (0.9 + Math.log2(node.count + 1) * 0.6) * 0.3;
          mesh.scale.setScalar(baseScale * shardScale);
          mesh.userData = { vtc, step: idx };
          scene.add(mesh);

          meshesRef.current[vtc] = { mesh, baseScale, pos: new THREE.Vector3(ox, oy, oz) };
        });

        // Build edge connections (temporal flow)
        keys.forEach((vtc, idx) => {
          if (idx < keys.length - 1) {
            const nextVtc = keys[idx + 1];
            edges.push({ from: vtc, to: nextVtc, step: idx });
          }
        });

        // Draw flow edges (solid tubes)
        const tubeRadius = 0.5;
        edges.forEach(({ from, to, step }) => {
          const fromMesh = meshesRef.current[from];
          const toMesh = meshesRef.current[to];
          if (!fromMesh || !toMesh) return;

          const curve = new THREE.LineCurve3(fromMesh.pos, toMesh.pos);
          const tubeGeom = new THREE.TubeGeometry(curve, 4, tubeRadius, 6, false);
          const mat = new THREE.MeshStandardMaterial({
            color: 0x00ff88,
            emissive: 0x00ff88,
            emissiveIntensity: 0.3,
            metalness: 0.7,
            roughness: 0.2,
          });
          const tube = new THREE.Mesh(tubeGeom, mat);
          tube.userData = { type: 'edge', step, from, to };
          scene.add(tube);
        });

        console.log(`[AppLite] ✓ All shards rendered (${keys.length} shards, ${edges.length} flows), starting animation...`);

        function animate() {
          requestAnimationFrame(animate);

          controls.update();

          Object.values(meshesRef.current).forEach(({ mesh }) => {
            if (!mesh.userData.vtc) return;
            mesh.rotation.x += 0.001 + (mesh.userData.step || 0) * 0.00001;
            mesh.rotation.y += 0.0015 + (mesh.userData.step || 0) * 0.000015;
          });

          // Animate pulse flowing through edges
          const time = Date.now() * 0.001;
          scene.children.forEach((child) => {
            if (child.userData?.type === 'edge') {
              const pulse = (time + child.userData.step * 0.1) % 1.0;
              const intensity = Math.max(0.1, 0.8 - Math.abs(pulse - 0.5) * 2);
              child.material.emissiveIntensity = intensity;
            }
          });

          renderer.render(scene, camera);
        }

        animate();
      })
      .catch((err) => {
        clearTimeout(loadTimeout);
        console.error("[AppLite] ❌ Error loading lattice:", err);
      });

    const handleResize = () => {
      const width = window.innerWidth;
      const height = window.innerHeight;
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      renderer.setSize(width, height);
    };

    window.addEventListener("resize", handleResize);

    return () => {
      console.log("[AppLite] Cleanup...");
      window.removeEventListener("resize", handleResize);
      ref.current?.removeChild(renderer.domElement);
    };
  }, [shardScale]);

  return (
    <>
      <div
        ref={ref}
        style={{
          width: "100vw",
          height: "100vh",
          margin: 0,
          padding: 0,
          overflow: "hidden",
          background: "#010408",
        }}
      >
        <div
          style={{
            position: "fixed",
            top: 16,
            right: 16,
            color: "rgba(0,255,136,0.4)",
            fontFamily: "monospace",
            fontSize: 11,
            textAlign: "right",
            pointerEvents: "none",
          }}
        >
          GN SHARD SPACE
          <br />
          <span style={{ color: "rgba(255,255,255,0.2)" }}>scroll · drag · click</span>
        </div>
      </div>

      <div style={{
        position: "fixed",
        bottom: 20,
        left: 20,
        background: "rgba(0,0,0,0.9)",
        padding: "12px 16px",
        borderRadius: "6px",
        color: "#0f0",
        fontFamily: "monospace",
        fontSize: "12px",
        border: "1px solid rgba(0,255,0,0.5)",
        zIndex: 1000,
        pointerEvents: "auto"
      }}>
        Size: {shardScale.toFixed(2)}x
        <input
          type="range"
          min="0.1"
          max="2"
          step="0.05"
          value={shardScale}
          onChange={(e) => setShardScale(parseFloat(e.target.value))}
          style={{ 
            display: "block", 
            marginTop: "8px", 
            width: "120px",
            accentColor: "#0f0",
            cursor: "pointer"
          }}
        />
      </div>
    </>
  );
}
