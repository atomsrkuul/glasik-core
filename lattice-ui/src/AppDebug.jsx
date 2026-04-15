import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { EffectComposer } from "three/examples/jsm/postprocessing/EffectComposer.js";
import { RenderPass } from "three/examples/jsm/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";

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
  const [selected, setSelected] = useState(null);
  const [debugLog, setDebugLog] = useState([]);

  const addLog = (msg) => {
    setDebugLog(prev => [...prev.slice(-20), `[${new Date().toLocaleTimeString()}] ${msg}`]);
    console.log(msg);
  };

  useEffect(() => {
    addLog("🔵 useEffect starting...");

    const W = window.innerWidth, H = window.innerHeight;
    addLog(`Canvas size: ${W} x ${H}`);

    const scene = new THREE.Scene();
    addLog("✓ Scene created");

    const camera = new THREE.PerspectiveCamera(60, W / H, 0.1, 5000);
    camera.position.set(0, 80, 600);
    addLog(`✓ Camera at (0, 80, 600)`);

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setClearColor(0x010408);
    addLog("✓ Renderer created");

    ref.current.innerHTML = "";
    ref.current.appendChild(renderer.domElement);
    addLog("✓ Renderer appended to DOM");

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(0, 0, 0);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 100;
    controls.maxDistance = 1200;
    controls.update();
    addLog("✓ OrbitControls initialized");

    // bloom
    const composer = new EffectComposer(renderer);
    composer.addPass(new RenderPass(scene, camera));
    const bloom = new UnrealBloomPass(new THREE.Vector2(W,H), 0.6, 0.4, 0.85);
    composer.addPass(bloom);
    addLog("✓ Bloom pass added");

    // lights
    scene.add(new THREE.AmbientLight(0x112233, 2));
    const dir = new THREE.DirectionalLight(0xffffff, 2.5);
    dir.position.set(80, 120, 60);
    scene.add(dir);
    const fill = new THREE.DirectionalLight(0x334466, 1);
    fill.position.set(-80, -40, -60);
    scene.add(fill);
    addLog("✓ Lights added");

    const meshes = {};
    const shardCenters = {};
    const shardData = {};
    const raycaster = new THREE.Raycaster();
    const mouse = new THREE.Vector2();

    addLog("📡 Fetching /lattice.json...");
    fetch("/lattice.json")
      .then(r => {
        addLog("✓ Fetch response received");
        return r.json();
      })
      .then(graph => {
        addLog(`✓ JSON parsed, ${Object.keys(graph).length} shards`);
        const keys = Object.keys(graph);
        const total = keys.length;

        addLog(`📍 Fibonacci sphere distribution (R=130):`);
        let minDist = Infinity, maxDist = 0;
        const positions = [];

        keys.forEach((vtc, idx) => {
          const node = graph[vtc];
          const color = TYPE_COLOR[node.type] || 0x44ff44;

          const phi = Math.acos(1 - (2 * (idx + 0.5)) / total);
          const theta = Math.PI * (1 + Math.sqrt(5)) * idx;
          const R = 130;

          const ox = R * Math.sin(phi) * Math.cos(theta);
          const oy = R * Math.sin(phi) * Math.sin(theta) - 5;
          const oz = R * Math.cos(phi);

          positions.push({ x: ox, y: oy, z: oz });

          const dist = Math.sqrt(ox*ox + oy*oy + oz*oz);
          minDist = Math.min(minDist, dist);
          maxDist = Math.max(maxDist, dist);

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
          const baseScale = (0.9 + Math.log2(node.count + 1) * 0.6) * 0.3;
          mesh.scale.setScalar(baseScale);
          mesh.userData = { vtc, color, node };
          scene.add(mesh);

          meshes[vtc] = mesh;
          shardCenters[vtc] = new THREE.Vector3(ox, oy, oz);
          shardData[vtc] = node;

          if (idx < 3) {
            addLog(`  [${idx}] ${vtc.slice(0, 12)}... → (${ox.toFixed(0)}, ${oy.toFixed(0)}, ${oz.toFixed(0)})`);
          }
        });

        addLog(`✓ ${total} shards rendered`);
        addLog(`  Distance range: ${minDist.toFixed(1)} to ${maxDist.toFixed(1)} from origin`);
        addLog(`  Camera at (0, 80, 600), distance from origin: ${Math.sqrt(0*0 + 80*80 + 600*600).toFixed(1)}`);

        // animation loop
        const animate = () => {
          requestAnimationFrame(animate);
          controls.update();
          Object.values(meshes).forEach(m => {
            m.rotation.x += 0.0005;
            m.rotation.y += 0.001;
          });
          composer.render();
        };

        addLog("▶️ Animation loop started");
        animate();

        // raycast
        document.addEventListener("click", (e) => {
          mouse.x = (e.clientX / W) * 2 - 1;
          mouse.y = -(e.clientY / H) * 2 + 1;
          raycaster.setFromCamera(mouse, camera);
          const hits = raycaster.intersectObjects(Object.values(meshes));
          if (hits.length > 0) {
            const vtc = hits[0].object.userData.vtc;
            setSelected(vtc);
            addLog(`✓ Selected: ${vtc}`);
          }
        });
      })
      .catch(err => {
        addLog(`❌ Error: ${err.message}`);
      });

    return () => {
      addLog("🔴 Cleanup");
      ref.current?.removeChild(renderer.domElement);
    };
  }, []);

  return (
    <div style={{ display: "flex", width: "100vw", height: "100vh" }}>
      <div
        ref={ref}
        style={{
          flex: 1,
          background: "#010408",
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
        }}
      >
        <div style={{ marginBottom: 10, fontWeight: "bold" }}>Debug Log</div>
        {debugLog.map((msg, i) => (
          <div key={i} style={{ marginBottom: 4, wordBreak: "break-all" }}>
            {msg}
          </div>
        ))}
        {selected && (
          <div style={{ marginTop: 20, padding: 10, background: "#111", border: "1px solid #0f0" }}>
            <div style={{ fontWeight: "bold" }}>Selected:</div>
            <div style={{ wordBreak: "break-all" }}>{selected}</div>
          </div>
        )}
      </div>
    </div>
  );
}
