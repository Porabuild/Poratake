import { cn } from '@/renderer/lib/utils';

const SEGMENT_COUNT = 32;

interface LevelMeterProps {
  level: number;
  active: boolean;
}

export default function LevelMeter({ level, active }: LevelMeterProps) {
  const filled = active ? Math.round(level * SEGMENT_COUNT) : 0;

  return (
    <div
      className="flex h-5 flex-1 items-center gap-0.5"
      role="meter"
      aria-label="Input level"
      aria-valuemin={0}
      aria-valuemax={1}
      aria-valuenow={active ? level : 0}
    >
      {Array.from({ length: SEGMENT_COUNT }, (_, index) => (
        <div
          key={index}
          className={cn(
            'h-full flex-1 rounded-sm transition-colors duration-75',
            index < filled ? 'bg-primary' : 'bg-muted'
          )}
        />
      ))}
    </div>
  );
}
