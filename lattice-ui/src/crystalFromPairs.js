import * as THREE from "three";

export function buildCrystalFromPairs(pairs, size = 2.6) {
  if (!pairs || pairs.length === 0) return null;

  // size passed from UI // 🔥 MASTER SIZE KNOB

  const points = [];

  pairs.forEach(({ lit, tok }) => {
    const angle = ((tok * 137) % 360) * (Math.PI / 180);
    const radius = Math.log2(2 + lit) * size;

    const x = Math.cos(angle) * radius;
    const y = Math.sin(angle) * radius;
    const z = (tok % 64) - 32;

    points.push(new THREE.Vector3(x, y, z));
  });

  const center = new THREE.Vector3();
  points.forEach(p => center.add(p));
  center.divideScalar(points.length);
  points.forEach(p => p.sub(center));

  if (points.length < 4) {
    return new THREE.SphereGeometry(2, 6, 6);
  }

  const geometry = new THREE.BufferGeometry();
  const vertices = [];

  for (let i = 2; i < points.length; i++) {
    const a = points[0];
    const b = points[i - 1];
    const c = points[i];

    vertices.push(a.x, a.y, a.z);
    vertices.push(b.x, b.y, b.z);
    vertices.push(c.x, c.y, c.z);
  }

  geometry.setAttribute(
    "position",
    new THREE.Float32BufferAttribute(vertices, 3)
  );

  geometry.computeVertexNormals();
  return geometry;
}
