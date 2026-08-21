import type { Rect } from '@/types/geometry';

export default function SelectionScrim({ rect }: { rect: Rect | null }) {
  if (!rect) {
    return <div className="pointer-events-none absolute inset-0 bg-black/50" />;
  }

  const right = rect.x + rect.width;
  const bottom = rect.y + rect.height;

  return (
    <div className="pointer-events-none absolute inset-0">
      <div
        className="absolute inset-x-0 top-0 bg-black/50"
        style={{ height: rect.y }}
      />
      <div
        className="absolute inset-x-0 bottom-0 bg-black/50"
        style={{ top: bottom }}
      />
      <div
        className="absolute left-0 bg-black/50"
        style={{ top: rect.y, height: rect.height, width: rect.x }}
      />
      <div
        className="absolute right-0 bg-black/50"
        style={{ top: rect.y, height: rect.height, left: right }}
      />
    </div>
  );
}
