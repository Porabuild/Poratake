import { Check } from 'lucide-react';
import { ASPECT_RATIOS } from '@/types/aspect-ratio';
import type { AspectRatio } from '@/types/aspect-ratio';

export default function AspectRatioMenu({
  activeRatio,
  onSelect,
}: {
  activeRatio: AspectRatio;
  onSelect: (ratio: AspectRatio) => void;
}) {
  return (
    <div className="absolute top-full left-1/2 mt-2 w-28 -translate-x-1/2 rounded-xl bg-black/85 p-1 shadow-lg backdrop-blur-md">
      {ASPECT_RATIOS.map(ratio => (
        <button
          key={ratio.name}
          type="button"
          className="flex w-full items-center justify-between rounded-lg px-3 py-1.5 text-sm text-white/90 transition-colors hover:bg-white/15 hover:text-white"
          onClick={() => onSelect(ratio)}
        >
          {ratio.name}
          {ratio.name === activeRatio.name ? (
            <Check className="size-3.5" />
          ) : null}
        </button>
      ))}
    </div>
  );
}
