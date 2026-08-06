import * as React from 'react';
import { Video, VideoOff } from 'lucide-react';
import { cn } from '@/renderer/lib/utils';

interface CameraButtonProps {
  enabled: boolean;
  onClick: () => void;
  title?: string;
}

export function CameraButton({ enabled, onClick, title }: CameraButtonProps) {
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
      <span className="relative z-10">
        {enabled ? (
          <Video size={18} strokeWidth={1.5} />
        ) : (
          <VideoOff size={18} strokeWidth={1.5} />
        )}
      </span>
    </button>
  );
}
