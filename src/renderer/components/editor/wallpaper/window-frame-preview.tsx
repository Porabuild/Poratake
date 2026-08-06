import type { WindowFrameStyle } from '@/types/editor';
import { cn } from '@/renderer/lib/utils';

interface WindowFramePreviewProps {
  style: WindowFrameStyle;
  isSelected: boolean;
  onClick: () => void;
}

interface TrafficLightProps {
  size?: number;
}

function TrafficLights({ size = 6 }: TrafficLightProps) {
  return (
    <div className="flex items-center gap-1">
      <div
        className="rounded-full"
        style={{
          width: size,
          height: size,
          backgroundColor: '#FF5F57',
        }}
      />
      <div
        className="rounded-full"
        style={{
          width: size,
          height: size,
          backgroundColor: '#FFBD2E',
        }}
      />
      <div
        className="rounded-full"
        style={{
          width: size,
          height: size,
          backgroundColor: '#28C840',
        }}
      />
    </div>
  );
}

export default function WindowFramePreview({
  style,
  isSelected,
  onClick,
}: WindowFramePreviewProps) {
  const isDark = style === 'macos-dark';
  const isNone = style === 'none';

  const getLabel = () => {
    switch (style) {
      case 'none':
        return 'None';
      case 'macos-light':
        return 'Light';
      case 'macos-dark':
        return 'Dark';
    }
  };

  return (
    <button
      onClick={onClick}
      className={cn(
        'flex w-full flex-col items-center gap-1.5 rounded-lg p-2 transition-all',
        isSelected
          ? 'bg-accent ring-ring ring-2 ring-offset-1'
          : 'hover:bg-accent/50'
      )}
    >
      <div
        className={cn(
          'flex h-12 w-full flex-col overflow-hidden rounded-md border',
          isNone && 'border-dashed',
          !isNone && isDark && 'border-neutral-700',
          !isNone && !isDark && 'border-neutral-300'
        )}
      >
        {!isNone && (
          <>
            {}
            <div
              className={cn(
                'flex items-center px-1.5 py-1',
                isDark ? 'bg-neutral-800' : 'bg-neutral-200'
              )}
            >
              <TrafficLights size={4} />
            </div>
            {}
            <div
              className={cn(
                'flex-1',
                isDark ? 'bg-neutral-900' : 'bg-neutral-50'
              )}
            />
          </>
        )}
        {isNone && (
          <div className="bg-muted/50 flex flex-1 items-center justify-center">
            <span className="text-muted-foreground text-[8px]">No frame</span>
          </div>
        )}
      </div>
      <span className="text-muted-foreground text-[10px]">{getLabel()}</span>
    </button>
  );
}
