import type { WindowFrameStyle } from '@/types/editor';
import { cn } from '@/renderer/lib/utils';
import {
  isWindowsFrame,
  WINDOW_FRAME_THEMES,
  type FramedWindowStyle,
} from '@/renderer/utils/window-frame';

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

function WindowsControls({ color }: { color: string }) {
  return (
    <div className="ml-auto flex h-full items-center">
      <span className="flex h-full w-3 items-center justify-center">
        <span className="h-px w-1.5" style={{ backgroundColor: color }} />
      </span>
      <span className="flex h-full w-3 items-center justify-center">
        <span className="size-1.5 border" style={{ borderColor: color }} />
      </span>
      <span className="relative flex h-full w-3 items-center justify-center">
        <span
          className="absolute h-px w-2 rotate-45"
          style={{ backgroundColor: color }}
        />
        <span
          className="absolute h-px w-2 -rotate-45"
          style={{ backgroundColor: color }}
        />
      </span>
    </div>
  );
}

export default function WindowFramePreview({
  style,
  isSelected,
  onClick,
}: WindowFramePreviewProps) {
  const isNone = style === 'none';
  const isWindows = isWindowsFrame(style);
  const theme = isNone ? null : WINDOW_FRAME_THEMES[style as FramedWindowStyle];

  const getLabel = () => {
    switch (style) {
      case 'none':
        return 'None';
      case 'macos-light':
        return 'macOS Light';
      case 'macos-dark':
        return 'macOS Dark';
      case 'windows-light':
        return 'Windows Light';
      case 'windows-dark':
        return 'Windows Dark';
    }
  };

  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={isSelected}
      className={cn(
        'focus-visible:ring-foreground flex w-full flex-col items-center gap-1.5 rounded-lg border p-2 transition-colors focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none',
        isSelected
          ? 'border-foreground bg-muted'
          : 'hover:border-border hover:bg-muted/50 border-transparent'
      )}
    >
      <div
        className={cn(
          'flex h-12 w-full flex-col overflow-hidden rounded-md border',
          isNone && 'border-border border-dashed'
        )}
        style={theme ? { borderColor: theme.frameBorder } : undefined}
      >
        {theme && (
          <>
            <div
              className="flex h-3.5 items-center px-1.5"
              style={{
                backgroundColor: theme.titleBar,
                borderBottom: `1px solid ${theme.titleBarBorder}`,
              }}
            >
              {isWindows ? (
                <WindowsControls color={theme.control} />
              ) : (
                <TrafficLights size={4} />
              )}
            </div>
            <div
              className="flex-1"
              style={{ backgroundColor: theme.content }}
            />
          </>
        )}
        {isNone && (
          <div className="bg-muted/50 flex flex-1 items-center justify-center">
            <span className="text-foreground text-xs">No frame</span>
          </div>
        )}
      </div>
      <span
        className={cn(
          'text-center text-xs leading-tight',
          isSelected ? 'text-foreground' : 'text-muted-foreground'
        )}
      >
        {getLabel()}
      </span>
    </button>
  );
}
