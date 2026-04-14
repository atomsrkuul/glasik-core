import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { EffectComposer } from "three/examples/jsm/postprocessing/EffectComposer.js";
import { RenderPass } from "three/examples/jsm/postprocessing/RenderPass.js";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";

function vtcToCrystal(vtc) {
  const hex = vtc.replace("VTC-v1-", "");
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) bytes.push(parseInt(hex.substr(i,2),16));
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
    points.push(new THREE.Vector3(Math.cos(angle)*radius, y, Math.sin(angle)*radius));
  }
  const lowerCount = upperCount + (bytes[9 % n] % 2);
  for (let i = 0; i < lowerCount; i++) {
    const b = bytes[(10 + i) % n];
    const angle = (i / lowerCount) * Math.PI * 2 + (b / 255) * 0.6;
    const radius = 4 + (b % 6);
    const y = -(1 + (bytes[(12 + i) % n] % 3));
    points.push(new THREE.Vector3(Math.cos(angle)*radius, y, Math.sin(angle)*radius));
  }
  const depth = -(5 + (bytes[n-1] % 6));
  points.push(new THREE.Vector3(0, depth, 0));
  return { points, upperCount, lowerCount };
}

function buildCrystalGeometry(vtc) {
  const { points, upperCount, lowerCount } = vtcToCrystal(vtc);
  const geo = new THREE.BufferGeometry();
  const verts = [], norms = [];
  const apex = points[0];
  const upper = points.slice(1, 1+upperCount);
  const lower = points.slice(1+upperCount, 1+upperCount+lowerCount);
  const base = points[points.length-1];
  function addTri(a,b,c) {
    const n = new THREE.Vector3().crossVectors(
      new THREE.Vector3().subVectors(b,a),
      new THREE.Vector3().subVectors(c,a)
    ).normalize();
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

const TYPE_COLOR = {
  user_intent:        0x00ff88,
  assistant_response: 0x2288ff,
  system_message:     0xff8800,
  code_block:         0xffee00,
  tool_call:          0xdd00ff,
  tool_result:        0x00eeff,
};

export default function App() {
  const ref = useRef(null);
  const [selected, setSelected] = useState(null);

  useEffect(() => {
    const W = window.innerWidth, H = window.innerHeight;
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x010408);
    scene.fog = new THREE.FogExp2(0x010408, 0.003);

    const camera = new THREE.PerspectiveCamera(60, W/H, 0.1, 5000);
    camera.position.set(0, 80, 600);

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(W, H);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.2;
    ref.current.innerHTML = "";
    ref.current.appendChild(renderer.domElement);

    // orbit controls
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.minDistance = 100;
    controls.maxDistance = 1200;

    // bloom
    const composer = new EffectComposer(renderer);
    composer.addPass(new RenderPass(scene, camera));
    const bloom = new UnrealBloomPass(new THREE.Vector2(W,H), 0.6, 0.4, 0.85);
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
    const shardCenters = {};
    const shardData = {};
    const raycaster = new THREE.Raycaster();
    const mouse = new THREE.Vector2();

    fetch("/lattice.json")
      .then(r => r.json())
      .then(graph => {
        const keys = Object.keys(graph);
        const total = keys.length;

        keys.forEach((vtc, idx) => {
          const node = graph[vtc];
          const color = TYPE_COLOR[node.type] || 0x44ff44;

          // fibonacci sphere
          const phi = Math.acos(1 - 2*(idx+0.5)/total);
          const theta = Math.PI*(1+Math.sqrt(5))*idx;
          const R = 130;
          const ox = R*Math.sin(phi)*Math.cos(theta);
          const oy = R*Math.sin(phi)*Math.sin(theta);
          const oz = R*Math.cos(phi);
          shardCenters[vtc] = new THREE.Vector3(ox,oy,oz);
          shardData[vtc] = { vtc, type: node.type, count: node.count, pairs: node.pairs?.length || 0 };

          const geo = buildCrystalGeometry(vtc);

          // main crystal
          const mat = new THREE.MeshPhongMaterial({
            color,
            emissive: new THREE.Color(color).multiplyScalar(0.2),
            shininess: 160,
            specular: 0xffffff,
            transparent: true,
            opacity: 0.88,
            side: THREE.DoubleSide,
          });
          const mesh = new THREE.Mesh(geo, mat);
          mesh.position.set(ox,oy,oz);
          const scale = 0.9 + node.count * 0.12;
          mesh.scale.setScalar(scale);
          mesh.userData = { vtc };
          scene.add(mesh);
          meshes[vtc] = mesh;

          // wireframe
          const wire = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({
            color: new THREE.Color(color).multiplyScalar(0.35),
            wireframe: true, transparent: true, opacity: 0.25
          }));
          wire.position.copy(mesh.position);
          wire.scale.copy(mesh.scale);
          scene.add(wire);

          // glow point at center
          const glowGeo = new THREE.SphereGeometry(0.6, 8, 8);
          const glowMat = new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.9 });
          const glow = new THREE.Mesh(glowGeo, glowMat);
          glow.position.set(ox,oy,oz);
          scene.add(glow);
        });

        // edges
        keys.forEach(vtc => {
          const from = shardCenters[vtc];
          Object.entries(graph[vtc].next||{}).forEach(([next, w]) => {
            const to = shardCenters[next];
            if (!to) return;
            const geo = new THREE.BufferGeometry().setFromPoints([from,to]);
            const mat = new THREE.LineBasicMaterial({
              color: 0x004466,
              transparent: true,
              opacity: Math.min(0.2 + w*0.15, 0.7)
            });
            scene.add(new THREE.Line(geo, mat));
          });
        });

        // click handler
        const onClick = (e) => {
          mouse.x = (e.clientX/window.innerWidth)*2-1;
          mouse.y = -(e.clientY/window.innerHeight)*2+1;
          raycaster.setFromCamera(mouse, camera);
          const hits = raycaster.intersectObjects(Object.values(meshes));
          if (hits.length > 0) {
            const vtc = hits[0].object.userData.vtc;
            setSelected(shardData[vtc]);
            // pulse selected
            Object.values(meshes).forEach(m => {
              m.material.emissive.set(
                m.userData.vtc === vtc
                  ? new THREE.Color(TYPE_COLOR[shardData[vtc]?.type]||0x44ff44).multiplyScalar(0.6)
                  : new THREE.Color(TYPE_COLOR[shardData[m.userData.vtc]?.type]||0x44ff44).multiplyScalar(0.2)
              );
            });
          }
        };
        renderer.domElement.addEventListener("click", onClick);

        let t = 0;
        let autoRotate = true;
        controls.addEventListener("start", () => { autoRotate = false; });

        function animate() {
          requestAnimationFrame(animate);
          if (autoRotate) {
            t += 0.003;
            scene.rotation.y = t * 0.3;
            scene.rotation.x = Math.sin(t*0.12)*0.15;
          }
          controls.update();
          composer.render();
        }
        animate();
      });

    const onResize = () => {
      camera.aspect = window.innerWidth/window.innerHeight;
      camera.updateProjectionMatrix();
      renderer.setSize(window.innerWidth, window.innerHeight);
      composer.setSize(window.innerWidth, window.innerHeight);
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      ref.current?.removeChild(renderer.domElement);
    };
  }, []);

  return (
    <>
      <div ref={ref} style={{ width:"100vw", height:"100vh" }} />
      {selected && (
        <div style={{
          position:"fixed", bottom:24, left:24,
          background:"rgba(0,8,16,0.85)",
          border:"1px solid rgba(0,255,136,0.3)",
          borderRadius:8, padding:"12px 18px",
          color:"#00ff88", fontFamily:"monospace", fontSize:13,
          backdropFilter:"blur(8px)", maxWidth:360,
          boxShadow:"0 0 20px rgba(0,255,136,0.15)"
        }}>
          <div style={{color:"#ffffff88",fontSize:11,marginBottom:6}}>CRYSTAL IDENTITY</div>
          <div style={{wordBreak:"break-all",marginBottom:4}}>{selected.vtc}</div>
          <div style={{color:"#ffffff66",marginTop:8,fontSize:11}}>
            type: <span style={{color:"#fff"}}>{selected.type}</span>
            &nbsp;·&nbsp;count: <span style={{color:"#fff"}}>{selected.count}</span>
            &nbsp;·&nbsp;pairs: <span style={{color:"#fff"}}>{selected.pairs}</span>
          </div>
        </div>
      )}
      <div style={{
        position:"fixed", top:16, right:16,
        color:"rgba(0,255,136,0.4)", fontFamily:"monospace", fontSize:11,
        textAlign:"right", pointerEvents:"none"
      }}>
        GN SHARD SPACE<br/>
        <span style={{color:"rgba(255,255,255,0.2)"}}>scroll · drag · click</span>
      </div>
    </>
  );
}
