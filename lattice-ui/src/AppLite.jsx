/**
 * AppLite - Lightweight lattice viewer with size slider
 * Loads lattice.json with timeout protection
 * Renders THREE.js canvas without heavy state management
 */

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

console.log("[AppLite] Component initializing...");

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

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.2;

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

    scene.add(new THREE.AmbientLight(0x112233, 2));

    const dir = new THREE.DirectionalLight(0xffffff, 2.5);
    dir.position.set(80, 120, 60);
    scene.add(dir);

    const fill = new THREE.DirectionalLight(0x334466, 1);
    fill.position.set(-80, -40, -60);
    scene.add(fill);

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
            flatShading: true,
            color,
            emissive: new THREE.Color(color).multiplyScalar(0.2),
            shininess: 220,
            specular: 0xffffff,
            transparent: false,
            opacity: 1.0,
            side: THREE.DoubleSide,
          });

          const mesh = new THREE.Mesh(geo, mat.clone());
          mesh.position.set(ox, oy, oz);

          const baseScale = (0.9 + Math.log2(node.count + 1) * 0.6) * 0.3;
          mesh.scale.setScalar(baseScale * shardScale);
          mesh.userData = { vtc, step: idx };
          scene.add(mesh);

          meshesRef.current[vtc] = { mesh, baseScale };
        });

        console.log("[AppLite] ✓ All shards rendered, starting animation...");

        function animate() {
          requestAnimationFrame(animate);

          controls.update();

          Object.values(meshesRef.current).forEach(({ mesh }) => {
            mesh.rotation.x += 0.001 + (mesh.userData.step || 0) * 0.00001;
            mesh.rotation.y += 0.0015 + (mesh.userData.step || 0) * 0.000015;
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
  }, []);

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
