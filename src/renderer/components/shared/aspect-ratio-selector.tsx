import { cn } from '@/renderer/lib/utils';
import type { AspectRatio } from '@/types/aspect-ratio';
import { ASPECT_RATIOS } from '@/types/aspect-ratio';

interface AspectRatioSelectorProps {
  value: AspectRatio | null;
  onChange: (ratio: AspectRatio | null) => void;
}

function isSelected(value: AspectRatio | null, ratio: AspectRatio): boolean {
  if (ratio.width === 0 && ratio.height === 0) {
    return value === null;
  }
  return value?.width === ratio.width && value?.height === ratio.height;
}

function getDisplayLabel(ratio: AspectRatio): string {
  if (ratio.width === 0 && ratio.height === 0) return 'Auto';
  return `${ratio.width}:${ratio.height}`;
}

export default function AspectRatioSelector({
  value,
  onChange,
}: AspectRatioSelectorProps) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-xs font-medium text-muted-foreground">
        Aspect Ratio
      </span>
      <div className="grid grid-cols-4 gap-1.5">
        {ASPECT_RATIOS.map(ratio => {
          const selected = isSelected(value, ratio);
          return (
            <button
              key={ratio.name}
              onClick={() =>
                onChange(ratio.width === 0 && ratio.height === 0 ? null : ratio)
              }
              className={cn(
                'rounded-md px-2 py-1.5 text-xs font-medium transition-colors',
                selected
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground'
              )}
            >
              {getDisplayLabel(ratio)}
            </button>
          );
        })}
      </div>
    </div>
  );
}
