export default function CrosshairGuides({ x, y }: { x: number; y: number }) {
  return (
    <div className="pointer-events-none absolute inset-0">
      <div
        className="absolute inset-y-0 w-px bg-primary/70"
        style={{ left: x }}
      />
      <div
        className="absolute inset-x-0 h-px bg-primary/70"
        style={{ top: y }}
      />
    </div>
  );
}
