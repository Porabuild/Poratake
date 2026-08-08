import { useCallback, useEffect, useRef, useState } from 'react';
import { Camera, ChevronDown, Ruler, X } from 'lucide-react';
import AspectRatioMenu from './aspect-ratio-menu';
import SizeEditor from './size-editor';
import ToolbarButton from './toolbar-button';
import type {
  AreaOverlayRect,
  AreaOverlayToolbarAction,
} from '@/types/area-overlay';
import { FREE_ASPECT_RATIO } from '@/types/aspect-ratio';
import type { AspectRatio } from '@/types/aspect-ratio';

type ToolbarPanel = 'ratio' | 'size' | null;

export default function AllInOneToolbar({
  recordingEnabled,
  rect,
  onAction,
}: {
  recordingEnabled: boolean;
  rect: AreaOverlayRect | null;
  onAction: (action: AreaOverlayToolbarAction) => void;
}) {
  const [openPanel, setOpenPanel] = useState<ToolbarPanel>(null);
  const [activeRatio, setActiveRatio] = useState(FREE_ASPECT_RATIO);
  const containerRef = useRef<HTMLDivElement>(null);
  const hasSelection = rect !== null;

  const switchPanel = useCallback(
    (next: ToolbarPanel) => {
      if (openPanel === next) return;
      if (openPanel === 'size') onAction({ action: 'size-editor-closed' });
      if (next === 'size') onAction({ action: 'size-editor-opened' });
      setOpenPanel(next);
    },
    [onAction, openPanel]
  );

  useEffect(() => {
    if (!openPanel) return;

    const handleMouseDown = (event: MouseEvent) => {
      if (containerRef.current?.contains(event.target as Node)) return;
      switchPanel(null);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.stopImmediatePropagation();
      switchPanel(null);
    };

    window.addEventListener('mousedown', handleMouseDown);
    window.addEventListener('keydown', handleKeyDown, true);

    return () => {
      window.removeEventListener('mousedown', handleMouseDown);
      window.removeEventListener('keydown', handleKeyDown, true);
    };
  }, [openPanel, switchPanel]);

  const selectRatio = (ratio: AspectRatio) => {
    setActiveRatio(ratio);
    switchPanel(null);
    onAction({
      action: 'select-aspect-ratio',
      name: ratio.name,
      width: ratio.width,
      height: ratio.height,
    });
  };

  const applySize = (size: { width: number; height: number }) => {
    switchPanel(null);
    onAction({ action: 'update-size', ...size });
  };

  return (
    <div
      ref={containerRef}
      className="absolute top-8 left-1/2 -translate-x-1/2 cursor-default"
      onMouseDown={event => event.stopPropagation()}
    >
      <div className="flex items-center gap-0.5 rounded-full bg-black/75 p-1 shadow-lg backdrop-blur-md">
        <ToolbarButton
          aria-label="Close"
          className="w-8 px-0"
          onClick={() => onAction({ action: 'close' })}
        >
          <X className="size-4" />
        </ToolbarButton>
        <div className="relative">
          <ToolbarButton onClick={() => switchPanel('ratio')}>
            {activeRatio.name}
            <ChevronDown className="size-3.5" />
          </ToolbarButton>
          {openPanel === 'ratio' ? (
            <AspectRatioMenu activeRatio={activeRatio} onSelect={selectRatio} />
          ) : null}
        </div>
        <div className="relative">
          <ToolbarButton
            disabled={!hasSelection}
            onClick={() => switchPanel('size')}
          >
            <Ruler className="size-4" />
            Size
          </ToolbarButton>
          {openPanel === 'size' && rect ? (
            <SizeEditor rect={rect} onApply={applySize} />
          ) : null}
        </div>
        <div className="h-5 w-px bg-white/20" />
        <ToolbarButton
          disabled={!hasSelection}
          onClick={() => onAction({ action: 'screenshot' })}
        >
          <Camera className="size-4" />
          Shot
        </ToolbarButton>
        {recordingEnabled ? (
          <ToolbarButton
            disabled={!hasSelection}
            onClick={() => onAction({ action: 'record' })}
          >
            <span className="size-2.5 rounded-full bg-red-500" />
            Rec
          </ToolbarButton>
        ) : null}
      </div>
    </div>
  );
}
