export default function ShapeNode({ data }) {
  const shape = data.shape || [];

  return (
    <div style={{
      width: 0,
      height: 0,
      overflow: "visible",
      background: "transparent",
      border: "none"
    }}>
      <svg width={160} height={160} style={{ overflow: "visible" }}>
        {shape.slice(0, 120).map((p, i) => {
          const depth = Math.abs(p.y) * 0.04;

          return (
            <circle
              key={i}
              cx={p.x}
              cy={-p.y}
              r={1.2 + depth}
              fill={`rgba(0,255,0,${0.5 + depth})`}
            />
          );
        })}
      </svg>
    </div>
  );
}
