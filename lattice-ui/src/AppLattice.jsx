import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { EffectComposer } from "three/examples/jsm/postprocessing/EffectComposer.js";
import { RenderPass } from "three/examples/jsm/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";

function vtcToCrystal(vtc) {
  const hex = vtc.replace("VTC-v1-", "").replace(/[^0-9a-f]/gi, "0").slice(0, 32);
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) bytes.push(parseInt(hex.substr(i, 2), 16));
  const n = Math.max(bytes.length, 1);
  const points = [];
  const height = 8 + (bytes[0] % 8);
  points.push(new THREE.Vector3(0, height, 0));

  const upperCount = 4 + (bytes[1 % n] % 3);
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

const TYPE_COLOR = {
  user: 0x00ff88,
  assistant: 0x2288ff,
};

export default function AppLattice({ latticeFile }) {
  const ref = useRef(null);
  const [shardCount, setShardCount] = useState(0);

  useEffect(() => {
    if (!ref.current) return;

    const W = window.innerWidth, H = window.innerHeight - 30;
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x010408);

    const camera = new THREE.PerspectiveCamera(60, W / H, 0.1, 5000);
    camera.position.set(0, 30, 280);

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);

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
    const bloom = new UnrealBloomPass(new THREE.Vector2(W, H), 0.6, 0.4, 0.85);
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

    fetch(latticeFile)
      .then(r => r.json())
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
          });
          const mesh = new THREE.Mesh(geo, mat);
          mesh.position.set(ox, oy, oz);
          const baseScale = (0.9 + Math.log2(node.count + 1) * 0.6) * 0.3;
          mesh.scale.setScalar(baseScale);
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
      .catch(err => console.error("Failed to load lattice:", err));

    return () => {
      if (ref.current?.children.length) ref.current.innerHTML = "";
    };
  }, [latticeFile]);

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <div ref={ref} style={{ width: "100%", height: "100%" }} />
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
