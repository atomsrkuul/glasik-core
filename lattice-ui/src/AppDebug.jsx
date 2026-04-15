import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

const TYPE_COLOR = {
  user_intent:        0x00ff88,
  assistant_response: 0x2288ff,
  system_message:     0xff8800,
  code_block:         0xffee00,
  tool_call:          0xdd00ff,
  tool_result:        0x00eeff,
};

function buildCrystalGeometry(vtc) {
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
    [a,b,c].forEach(p => { verts.push(p.x,p.y,p.z); norms.push(n.x,n.y,n.z); });
  }
  for (let i=0;i<upperCount;i++) addTri(apex,upper[i],upper[(i+1)%upperCount]);
  for (let i=0;i<upperCount;i++) {
    const u0=upper[i],u1=upper[(i+1)%upperCount];
    const l0=lower[i%lowerCount],l1=lower[(i+1)%lowerCount];
    addTri(u0,l0,u1); addTri(u1,l0,l1);
  }
  for (let i=0;i<lowerCount;i++) addTri(base,lower[(i+1)%lowerCount],lower[i]);
  geo.setAttribute("position",new THREE.Float32BufferAttribute(verts,3));
  geo.setAttribute("normal",new THREE.Float32BufferAttribute(norms,3));
  return geo;
}

export default function AppDebug() {
  const ref = useRef(null);
  const [debugLog, setDebugLog] = useState([]);

  const addLog = (msg) => {
    const time = new Date().toLocaleTimeString();
    const full = `[${time}] ${msg}`;
    setDebugLog(prev => [...prev.slice(-30), full]);
    console.log(full);
  };

  useEffect(() => {
    const W = window.innerWidth, H = window.innerHeight;
    addLog(`Canvas ${W}x${H}`);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x000000);
    addLog("Scene created");

    const camera = new THREE.PerspectiveCamera(60, W / H, 0.1, 5000);
    camera.position.set(0, 80, 600);
    addLog("Camera @ (0,80,600)");

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setClearColor(0x000000);
    addLog("Renderer ready");

    ref.current.innerHTML = "";
    ref.current.appendChild(renderer.domElement);
    addLog("Canvas mounted");

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(0, 0, 0);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 100;
    controls.maxDistance = 1200;
    controls.update();
    addLog("Orbit controls ready");

    // Bright lights
    scene.add(new THREE.AmbientLight(0xffffff, 2));
    const dir = new THREE.DirectionalLight(0xffffff, 3.5);
    dir.position.set(200, 200, 200);
    scene.add(dir);
    const fill = new THREE.DirectionalLight(0xffffff, 2);
    fill.position.set(-200, -200, -200);
    scene.add(fill);
    addLog("3x bright lights");

    const meshes = {};
    const raycaster = new THREE.Raycaster();
    const mouse = new THREE.Vector2();

    addLog("Fetching /lattice.json...");

    fetch("/lattice.json")
      .then(r => r.json())
      .then(graph => {
        const keys = Object.keys(graph);
        const total = keys.length;
        addLog(`Loaded ${total} shards`);

        let minDist = Infinity, maxDist = 0;

        keys.forEach((vtc, idx) => {
          const node = graph[vtc];
          const color = TYPE_COLOR[node.type] || 0x44ff44;

          const phi = Math.acos(1 - (2 * (idx + 0.5)) / total);
          const theta = Math.PI * (1 + Math.sqrt(5)) * idx;
          const R = 150;

          const ox = R * Math.sin(phi) * Math.cos(theta);
          const oy = R * Math.sin(phi) * Math.sin(theta);
          const oz = R * Math.cos(phi);

          const dist = Math.sqrt(ox*ox + oy*oy + oz*oz);
          minDist = Math.min(minDist, dist);
          maxDist = Math.max(maxDist, dist);

          const geo = buildCrystalGeometry(vtc);
          const mat = new THREE.MeshPhongMaterial({
            color,
            emissive: new THREE.Color(color).multiplyScalar(0.3),
            shininess: 220,
            specular: 0xffffff,
            side: THREE.DoubleSide,
            flatShading: true,
          });
          const mesh = new THREE.Mesh(geo, mat);
          mesh.position.set(ox, oy, oz);
          mesh.visible = true;
          const baseScale = (0.9 + Math.log2(node.count + 1) * 0.6) * 0.4;
          mesh.scale.setScalar(baseScale);
          mesh.userData = { vtc, color, index: idx };
          scene.add(mesh);
          meshes[vtc] = mesh;

          if (idx < 3) {
            addLog(`  [${idx}] @ (${ox.toFixed(0)}, ${oy.toFixed(0)}, ${oz.toFixed(0)})`);
          }
        });

        addLog(`Shards at R=${minDist.toFixed(0)}-${maxDist.toFixed(0)}`);
        addLog(`Scene: ${scene.children.length} objects`);
        addLog(`Meshes: ${Object.keys(meshes).length} visible`);

        let frameCount = 0;
        const animate = () => {
          frameCount++;
          if (frameCount === 1) {
            addLog("Animation started!");
          }
          requestAnimationFrame(animate);
          controls.update();
          
          Object.values(meshes).forEach(m => {
            m.rotation.x += 0.0005;
            m.rotation.y += 0.001;
          });

          renderer.render(scene, camera);
        };

        // Expose for debugging
        window.DEBUG = { scene, camera, renderer, meshes, controls };

        animate();
      })
      .catch(err => {
        addLog(`ERROR: ${err.message}`);
        console.error(err);
      });

    return () => {
      addLog("Cleanup");
      if (ref.current?.children.length) {
        ref.current.innerHTML = "";
      }
    };
  }, []);

  return (
    <div style={{ display: "flex", width: "100vw", height: "100vh", overflow: "hidden" }}>
      <div
        ref={ref}
        style={{
          flex: 1,
          background: "#000",
          position: "relative",
        }}
      />
      <div
        style={{
          width: 300,
          background: "#000",
          color: "#0f0",
          fontFamily: "monospace",
          fontSize: 11,
          padding: 10,
          overflow: "auto",
          borderLeft: "2px solid #0f0",
          boxSizing: "border-box",
        }}
      >
        <div style={{ marginBottom: 10, fontWeight: "bold", position: "sticky", top: 0, background: "#000", zIndex: 10 }}>
          Debug Log
        </div>
        {debugLog.map((msg, i) => (
          <div key={i} style={{ marginBottom: 2, wordBreak: "break-word", fontSize: "10px", lineHeight: "1.2", color: msg.includes("ERROR") ? "#f00" : "#0f0" }}>
            {msg}
          </div>
        ))}
      </div>
    </div>
  );
}
