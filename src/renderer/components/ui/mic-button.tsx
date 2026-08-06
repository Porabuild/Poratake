import * as React from 'react';
import { Mic, MicOff } from 'lucide-react';
import { cn } from '@/renderer/lib/utils';
import { useAudioLevel } from '@/renderer/hooks/useAudioLevel';

interface MicButtonProps {
  enabled: boolean;
  deviceId: string | null;
  onClick: () => void;
  title?: string;
}

export function MicButton({
  enabled,
  deviceId,
  onClick,
  title,
}: MicButtonProps) {
  const audioLevel = useAudioLevel({
    deviceId,
    enabled,
    smoothingFactor: 0.4,
  });

  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      className={cn(
        'relative flex h-full w-full items-center justify-center overflow-hidden transition-all duration-200',
        'text-muted-foreground hover:text-foreground',
        enabled && 'text-foreground'
      )}
      style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
    >
      {}
      {enabled && (
        <div className="bg-accent pointer-events-none absolute inset-0" />
      )}

      {}
      {enabled && (
        <div
          className="bg-accent pointer-events-none absolute inset-0 transition-transform duration-75"
          style={{
            transform: `scaleY(${audioLevel})`,
            transformOrigin: 'center bottom',
            opacity: 0.5,
          }}
        />
      )}

      {}
      <span className="relative z-10">
        {enabled ? (
          <Mic size={18} strokeWidth={1.5} />
        ) : (
          <MicOff size={18} strokeWidth={1.5} />
        )}
      </span>
    </button>
  );
}
