/**
 * Debug version - minimal canvas test
 */

import { useEffect, useRef } from "react";
import * as THREE from "three";

export default function AppDebug() {
  const ref = useRef(null);

  useEffect(() => {
    if (!ref.current) {
      console.error("❌ ref.current is null");
      return;
    }

    console.log("✓ ref.current exists:", ref.current);

    // Minimal Three.js scene
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x111111);

    const camera = new THREE.PerspectiveCamera(
      75,
      window.innerWidth / window.innerHeight,
      0.1,
      1000
    );
    camera.position.z = 5;

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(window.innerWidth, window.innerHeight);
    renderer.setPixelRatio(window.devicePixelRatio);

    console.log("✓ Renderer created, adding to DOM");
    ref.current.appendChild(renderer.domElement);

    // Add a simple cube
    const geometry = new THREE.BoxGeometry(2, 2, 2);
    const material = new THREE.MeshPhongMaterial({ color: 0x7c3aed });
    const cube = new THREE.Mesh(geometry, material);
    scene.add(cube);

    // Add light
    const light = new THREE.DirectionalLight(0xffffff, 1);
    light.position.set(5, 5, 5);
    scene.add(light);

    // Animation loop
    function animate() {
      requestAnimationFrame(animate);
      cube.rotation.x += 0.01;
      cube.rotation.y += 0.01;
      renderer.render(scene, camera);
    }

    console.log("✓ Starting animation");
    animate();

    // Handle resize
    const handleResize = () => {
      const width = window.innerWidth;
      const height = window.innerHeight;
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      renderer.setSize(width, height);
    };

    window.addEventListener("resize", handleResize);

    return () => {
      window.removeEventListener("resize", handleResize);
      ref.current?.removeChild(renderer.domElement);
    };
  }, []);

  return (
    <div
      ref={ref}
      style={{
        width: "100vw",
        height: "100vh",
        margin: 0,
        padding: 0,
        overflow: "hidden",
        background: "#111"
      }}
    >
      <div style={{
        position: "fixed",
        top: 10,
        left: 10,
        color: "#7c3aed",
        fontFamily: "monospace",
        fontSize: "12px",
        zIndex: 10
      }}>
        🟣 Debug Canvas Test
        <div style={{ fontSize: "10px", color: "#888", marginTop: "8px" }}>
          Check browser console for logs
        </div>
      </div>
    </div>
  );
}
