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

const TYPE_COLOR = {
  user_intent: 0x00ff88,
  assistant_response: 0x2288ff,
  system_message: 0xff8800,
  code_block: 0xffee00,
  tool_call: 0xdd00ff,
  tool_result: 0x00eeff,
};

function makeArrow(from, to, color) {
  const dir = new THREE.Vector3().subVectors(to, from).normalize();
  const len = from.distanceTo(to);
  const arrowPos = new THREE.Vector3().lerpVectors(from, to, 0.8);
  return new THREE.ArrowHelper(dir, arrowPos, len * 0.06, color, len * 0.04, len * 0.025);
}

export default function App() {
  const ref = useRef(null);
  const [selected, setSelected] = useState(null);
  const [hoveredEdge, setHoveredEdge] = useState(null);
  const [playhead, setPlayhead] = useState(null);
  const [maxStep, setMaxStep] = useState(0);
  const [crystalSize, setCrystalSize] = useState(1.0);

  const playheadRef = useRef(null);
  const meshesRef = useRef({});
  const stepsRef = useRef([]);

  useEffect(() => {
    const W = window.innerWidth;
    const H = window.innerHeight;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x010408);
    scene.fog = new THREE.FogExp2(0x010408, 0.003);

    const camera = new THREE.PerspectiveCamera(60, W / H, 0.1, 5000);
    camera.position.set(0, 30, 280);

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.2;

    ref.current.innerHTML = "";
    ref.current.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 50;
    controls.maxDistance = 600;

    const composer = new EffectComposer(renderer);
    composer.addPass(new RenderPass(scene, camera));
    const bloom = new UnrealBloomPass(new THREE.Vector2(W, H), 0.7, 0.4, 0.82);
    composer.addPass(bloom);

    scene.add(new THREE.AmbientLight(0x112233, 2));

    const dir = new THREE.DirectionalLight(0xffffff, 2.5);
    dir.position.set(80, 120, 60);
    scene.add(dir);

    const fill = new THREE.DirectionalLight(0x334466, 1);
    fill.position.set(-80, -40, -60);
    scene.add(fill);

    const meshes = {};
        const pickables = [];
    const shardCenters = {};
    const shardData = {};
    const particles = [];
    const edgeMeshes = [];
    const raycaster = new THREE.Raycaster();
    const mouse = new THREE.Vector2();
    let autoRotate = true;

    controls.addEventListener("start", () => {
      autoRotate = false;
    });

    fetch("/lattice.json")
      .then((r) => r.json())
      .then((graph) => {
        const keys = Object.keys(graph);
        const total = keys.length;
        stepsRef.current = keys;
        setMaxStep(total - 1);
        setPlayhead(total - 1);
        playheadRef.current = total - 1;

        keys.forEach((vtc, idx) => {
          const node = graph[vtc];
          const color = TYPE_COLOR[node.type] || 0x44ff44;

          const phi = Math.acos(1 - (2 * (idx + 0.5)) / total);
          const theta = Math.PI * (1 + Math.sqrt(5)) * idx;
          const R = 130;

          const ox = R * Math.sin(phi) * Math.cos(theta);
          const oy = R * Math.sin(phi) * Math.sin(theta);
          const oz = R * Math.cos(phi);

          shardCenters[vtc] = new THREE.Vector3(ox, oy, oz);
          shardData[vtc] = {
            vtc,
            type: node.type,
            count: node.count,
            pairs: node.pairs?.length || 0,
            step: idx,
          };

          const geo = buildCrystalFromPairs(node.pairs || []) || buildCrystalGeometry(vtc);
          const mat = new THREE.MeshPhongMaterial({
            flatShading: true,
            color,
            emissive: new THREE.Color(color).multiplyScalar(0.2),
            shininess: 220,
            specular: 0xffffff,
            transparent: true,
            opacity: 0.88,
            side: THREE.DoubleSide,
          });

          const mesh = new THREE.Mesh(geo, mat);
          mesh.position.set(ox, oy, oz);

          const scale = 0.9 + Math.log2(node.count + 1) * 0.6;
          mesh.scale.setScalar(scale * crystalSize * 1.5);

          mesh.userData = { vtc, step: idx };
          scene.add(mesh);

          const pickMesh = new THREE.Mesh(
            new THREE.SphereGeometry(5, 12, 12),
            new THREE.MeshBasicMaterial({ visible: false })
          );
          pickMesh.position.copy(mesh.position);
          pickMesh.userData = { vtc };
          scene.add(pickMesh);
          pickables.push(pickMesh);

          const clickShell = new THREE.Mesh(
            new THREE.SphereGeometry(4, 12, 12),
            new THREE.MeshBasicMaterial({ visible: false })
          );
          clickShell.userData = { vtc, step: idx };
          mesh.add(clickShell);

          const hitGeo = new THREE.SphereGeometry(6, 8, 8);
          const hitMat = new THREE.MeshBasicMaterial({ visible: false });
          const hitMesh = new THREE.Mesh(hitGeo, hitMat);
          hitMesh.position.copy(mesh.position);
          hitMesh.userData = mesh.userData;
          scene.add(hitMesh);

          meshes[vtc] = mesh;
          meshesRef.current = meshes;

          const wire = new THREE.Mesh(
            geo,
            new THREE.MeshBasicMaterial({
              color: new THREE.Color(color).multiplyScalar(0.35),
              wireframe: true,
              transparent: true,
              opacity: 0.25,
            })
          );
          wire.position.copy(mesh.position);
          wire.scale.copy(mesh.scale);
          scene.add(wire);

          const glowGeo = new THREE.SphereGeometry(0.6, 8, 8);
          const glow = new THREE.Mesh(
            glowGeo,
            new THREE.MeshBasicMaterial({
              color,
              transparent: true,
              opacity: 0.9,
            })
          );
          glow.position.set(ox, oy, oz);
          scene.add(glow);
        });

        keys.forEach((vtc, fromIdx) => {
          const from = shardCenters[vtc];

          Object.entries(graph[vtc].next || {}).filter(([_, weight]) => weight >= 1).forEach(([next, weight]) => {
            const to = shardCenters[next];
            if (!to) return;

            const toIdx = keys.indexOf(next);
            const color = TYPE_COLOR[graph[vtc].type] || 0x004466;

            const path = new THREE.QuadraticBezierCurve3(from, from.clone().lerp(to, 0.5).add(new THREE.Vector3(0, 6 + weight * 6, 0)), to);
            const tubeGeo = new THREE.TubeGeometry(path, 1, Math.min(0.25, 0.03 + weight * 0.05), 6, false);
            const mat = new THREE.MeshBasicMaterial({
              color: new THREE.Color().setHSL(0.6 - Math.min(weight * 0.08, 0.4), 0.8, 0.5),
              transparent: true,
              opacity: Math.min(0.15 + weight * 0.12, 0.6),
            });

            const line = new THREE.Mesh(tubeGeo, mat);
            line.userData = { from: vtc, to: next, weight, fromIdx, toIdx };
            scene.add(line);
            edgeMeshes.push(line);

            line.userData.pulse = Math.random() * Math.PI * 2;

            const arrow = makeArrow(from, to, color);
            arrow.userData = { fromIdx, toIdx };
            scene.add(arrow);

            const particleCount = Math.min(weight + 1, 4);
            for (let p = 0; p < particleCount; p++) {
              const pGeo = new THREE.SphereGeometry(0.3 + weight * 0.2, 4, 4);
              const pMat = new THREE.MeshBasicMaterial({
                color: TYPE_COLOR[graph[vtc].type] || 0x00ff88,
                transparent: true,
                opacity: 0.8,
              });

              const pMesh = new THREE.Mesh(pGeo, pMat);
              scene.add(pMesh);

              particles.push({
                mesh: pMesh,
                from,
                to,
                speed: 0.003 + weight * 0.004,
                t: p / particleCount,
                fromIdx,
                toIdx,
              });
            }
          });
        });

        renderer.domElement.addEventListener("click", (e) => {
          const rect = renderer.domElement.getBoundingClientRect();
          mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
          mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
          raycaster.setFromCamera(mouse, camera);
          raycaster.params.Mesh = raycaster.params.Mesh || {};
          raycaster.params.Mesh.threshold = 2;
          const hits = raycaster.intersectObjects(pickables, false);

          if (hits.length > 0) {
            const vtc = hits[0].object.userData.vtc;
            setSelected(shardData[vtc]);

            Object.values(meshes).forEach((m) => {
              if (!m.material) return;
              const isSelected = m.userData.vtc === vtc;
              m.material.emissive.setScalar(isSelected ? 0.9 : 0.0);
            });
          }
        });

        renderer.domElement.addEventListener("mousemove", (e) => {
          mouse.x = (e.clientX / window.innerWidth) * 2 - 1;
          mouse.y = -(e.clientY / window.innerHeight) * 2 + 1;
          raycaster.setFromCamera(mouse, camera);
          raycaster.params.Line.threshold = 2;
          const hits = raycaster.intersectObjects(edgeMeshes);

          if (hits.length > 0) {
            const d = hits[0].object.userData;
            setHoveredEdge({ from: d.from, to: d.to, weight: d.weight });
          } else {
            setHoveredEdge(null);
          }
        });

        let t = 0;
        function animate() {
          requestAnimationFrame(animate);
          t += 0.003;

          if (autoRotate) {
            scene.rotation.y = t * 0.3;
            scene.rotation.x = Math.sin(t * 0.12) * 0.15;
          }

          const ph = playheadRef.current;

          particles.forEach((p) => {
            const visible = p.fromIdx <= ph && p.toIdx <= ph;
            p.mesh.visible = visible;
            if (!visible) return;

            p.t = (p.t + p.speed) % 1;
            p.mesh.position.lerpVectors(p.from, p.to, p.t);
            const fade = Math.pow(Math.sin(p.t * Math.PI), 1.5);
            p.mesh.material.opacity = fade * 0.9;
          });

          Object.values(meshes).forEach((m) => {
            m.visible = m.userData.step <= ph;
          });

          controls.update();
          composer.render();
        }

        animate();
      });

    const onResize = () => {
      camera.aspect = window.innerWidth / window.innerHeight;
      camera.updateProjectionMatrix();
      renderer.setSize(window.innerWidth, window.innerHeight);
      composer.setSize(window.innerWidth, window.innerHeight);
    };

    window.addEventListener("resize", onResize);

    return () => {
      window.removeEventListener("resize", onResize);
      ref.current?.removeChild(renderer.domElement);
    };
  }, [crystalSize]);

  const handleSlider = (e) => {
    const v = parseInt(e.target.value);
    setPlayhead(v);
    playheadRef.current = v;
  };

  return (
    <>
      <div ref={ref} style={{ width: "100vw", height: "100vh" }} />

      {selected && (
        <div
          style={{
            position: "fixed",
            bottom: 80,
            left: 24,
            background: "rgba(0,8,16,0.88)",
            border: "1px solid rgba(0,255,136,0.3)",
            borderRadius: 8,
            padding: "12px 18px",
            color: "#00ff88",
            fontFamily: "monospace",
            fontSize: 13,
            backdropFilter: "blur(8px)",
            maxWidth: 360,
            boxShadow: "0 0 20px rgba(0,255,136,0.15)",
          }}
        >
          <div style={{ color: "#ffffff88", fontSize: 11, marginBottom: 6 }}>CRYSTAL IDENTITY</div>
          <div style={{ wordBreak: "break-all", marginBottom: 4 }}>{selected.vtc}</div>
          <div style={{ color: "#ffffff66", marginTop: 8, fontSize: 11 }}>
            type: <span style={{ color: "#fff" }}>{selected.type}</span>
            &nbsp;·&nbsp;count: <span style={{ color: "#fff" }}>{selected.count}</span>
            &nbsp;·&nbsp;pairs: <span style={{ color: "#fff" }}>{selected.pairs}</span>
            &nbsp;·&nbsp;step: <span style={{ color: "#fff" }}>{selected.step}</span>
          </div>
        </div>
      )}

      {hoveredEdge && (
        <div
          style={{
            position: "fixed",
            top: "50%",
            left: "50%",
            transform: "translate(-50%,-50%)",
            background: "rgba(0,8,20,0.9)",
            border: "1px solid rgba(0,180,255,0.4)",
            borderRadius: 6,
            padding: "8px 14px",
            color: "#00ccff",
            fontFamily: "monospace",
            fontSize: 12,
            pointerEvents: "none",
            boxShadow: "0 0 12px rgba(0,180,255,0.2)",
          }}
        >
          <div style={{ color: "#ffffff55", fontSize: 10, marginBottom: 4 }}>TRANSITION</div>
          <div style={{ fontSize: 10, color: "#fff88" }}>
            {hoveredEdge.from?.slice(0, 16)}... → {hoveredEdge.to?.slice(0, 16)}...
          </div>
          <div style={{ marginTop: 4 }}>
            weight: <span style={{ color: "#fff" }}>{hoveredEdge.weight}</span>
          </div>
        </div>
      )}

      <div
        style={{
          position: "fixed",
          bottom: 24,
          left: "50%",
          transform: "translateX(-50%)",
          background: "rgba(0,8,16,0.85)",
          border: "1px solid rgba(255,255,255,0.1)",
          borderRadius: 8,
          padding: "10px 20px",
          fontFamily: "monospace",
          fontSize: 11,
          color: "rgba(255,255,255,0.5)",
          backdropFilter: "blur(8px)",
          display: "flex",
          alignItems: "center",
          gap: 12,
        }}
      >
        <span style={{ color: "rgba(0,255,136,0.6)" }}>FORMATION</span>
        <input
          type="range"
          min={0}
          max={maxStep}
          value={playhead ?? maxStep}
          onChange={handleSlider}
          style={{ width: 200, accentColor: "#00ff88" }}
        />

        <span style={{ color: "rgba(0,180,255,0.6)", marginLeft: 16 }}>SIZE</span>
        <input
          type="range"
          min={0.5}
          max={2.0}
          step={0.05}
          value={crystalSize}
          onChange={(e) => setCrystalSize(parseFloat(e.target.value))}
          style={{ width: 120, accentColor: "#00ccff" }}
        />

        <span style={{ color: "#fff" }}>
          {(playhead ?? maxStep) + 1} / {maxStep + 1}
        </span>
      </div>

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
    </>
  );
}

function buildCrystalFromPairs(pairs) {
  if (!pairs || pairs.length < 3) {
    return new THREE.SphereGeometry(0.5, 6, 6);
  }

  const pointsVec = [];

  const freq = {};
  pairs.forEach(({ lit }) => {
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

  pairs.forEach(({ lit, tok }, i) => {
    for (let d = 0; d < densityFactor; d++) {
    const angle = (tok * 0.3) + (lit % 11) * 0.25 + Math.sin(tok * 0.7) * 0.5;
    const radius = Math.min(2.5, Math.log2(lit + 1));

    const skewX = 0.6 + (lit % 5) * 0.1;
    const skewY = 0.6 + (tok % 7) * 0.08;
    const skewZ = 0.4 + ((lit + tok) % 9) * 0.06;

    const x = Math.cos(angle) * radius * skewX * (0.8 + heightScale * 0.2);
    const y = Math.sin(angle) * radius * skewY * (0.8 + heightScale * 0.2);
    const z = tok * 0.05 * skewZ * (0.6 + heightScale * 0.6);

    pointsVec.push(new THREE.Vector3(x, y, z));
    }
  });

  const geometry = new ConvexGeometry(pointsVec);

  geometry.computeBoundingBox();
  const center = new THREE.Vector3();
  geometry.boundingBox.getCenter(center);
  geometry.translate(-center.x, -center.y, -center.z);

  
  geometry.computeBoundingSphere();
  const radius = geometry.boundingSphere.radius || 1;
  const scale = 4.5 / radius;
  geometry.scale(scale, scale, scale);

  return geometry;
}

