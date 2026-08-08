export default function CrosshairGuides({ x, y }: { x: number; y: number }) {
  return (
    <div className="pointer-events-none absolute inset-0">
      <div
        className="bg-primary/70 absolute inset-y-0 w-px"
        style={{ left: x }}
      />
      <div
        className="bg-primary/70 absolute inset-x-0 h-px"
        style={{ top: y }}
      />
    </div>
  );
}
