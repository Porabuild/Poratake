import { useState } from 'react';
import type { AreaOverlayRect } from '@/types/area-overlay';

const inputClass =
  'h-8 w-16 rounded-lg bg-white/10 px-2 text-center text-sm text-white outline-none transition-colors focus:bg-white/20';

export default function SizeEditor({
  rect,
  onApply,
}: {
  rect: AreaOverlayRect;
  onApply: (size: { width: number; height: number }) => void;
}) {
  const [width, setWidth] = useState(String(rect.width));
  const [height, setHeight] = useState(String(rect.height));

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();

    const parsedWidth = Number.parseInt(width, 10);
    const parsedHeight = Number.parseInt(height, 10);

    if (
      !Number.isFinite(parsedWidth) ||
      !Number.isFinite(parsedHeight) ||
      parsedWidth <= 0 ||
      parsedHeight <= 0
    ) {
      return;
    }

    onApply({ width: parsedWidth, height: parsedHeight });
  };

  return (
    <form
      className="absolute top-full left-1/2 mt-2 flex -translate-x-1/2 items-center gap-1.5 rounded-xl bg-black/85 p-1.5 shadow-lg backdrop-blur-md"
      onSubmit={handleSubmit}
    >
      <input
        autoFocus
        inputMode="numeric"
        className={inputClass}
        value={width}
        onChange={event => setWidth(event.target.value)}
        aria-label="Width"
      />
      <span className="text-sm text-white/60">×</span>
      <input
        inputMode="numeric"
        className={inputClass}
        value={height}
        onChange={event => setHeight(event.target.value)}
        aria-label="Height"
      />
      <button
        type="submit"
        className="h-8 rounded-lg bg-white/90 px-3 text-sm font-medium text-black transition-colors hover:bg-white"
      >
        Apply
      </button>
    </form>
  );
}
