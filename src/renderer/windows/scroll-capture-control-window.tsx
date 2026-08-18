import { useCallback, useEffect, useState } from 'react';
import { Check, Play, Square, X } from 'lucide-react';
import ToolbarButton from '@/renderer/components/area-overlay/toolbar-button';
import ToolbarSurface from '@/renderer/components/area-overlay/toolbar-surface';
import { EMPTY_SCROLL_CAPTURE_STATE } from '@/types/scroll-capture';
import type {
  ScrollCaptureAction,
  ScrollCaptureOverlayState,
} from '@/types/scroll-capture';

export default function ScrollCaptureControlWindow() {
  const [state, setState] = useState<ScrollCaptureOverlayState>(
    EMPTY_SCROLL_CAPTURE_STATE
  );

  useEffect(() => {
    const handleUpdate = (_event: unknown, update: ScrollCaptureOverlayState) =>
      setState(update);
    window.ipcRenderer.on('scroll-capture:update', handleUpdate);
    return () => {
      window.ipcRenderer.off('scroll-capture:update', handleUpdate);
    };
  }, []);

  const sendAction = useCallback((action: ScrollCaptureAction) => {
    window.ipcRenderer.send('scroll-capture:action', action);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        sendAction('cancel');
      } else if (event.key === 'Enter') {
        sendAction('done');
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [sendAction]);

  const autoScrollLabel = state.isAutoScrolling
    ? 'Stop auto-scroll'
    : 'Start auto-scroll';

  return (
    <div className="flex h-screen w-screen items-center justify-center">
      <ToolbarSurface>
        <ToolbarButton
          aria-label={autoScrollLabel}
          title={autoScrollLabel}
          onClick={() => sendAction('toggle-auto-scroll')}
        >
          {state.isAutoScrolling ? (
            <Square className="size-3.5 fill-current" />
          ) : (
            <Play className="size-3.5 fill-current" />
          )}
        </ToolbarButton>
        <ToolbarButton
          aria-label="Done (Enter)"
          title="Done (Enter)"
          onClick={() => sendAction('done')}
        >
          <Check className="size-4" />
        </ToolbarButton>
        <ToolbarButton
          aria-label="Cancel (Esc)"
          title="Cancel (Esc)"
          onClick={() => sendAction('cancel')}
        >
          <X className="size-4" />
        </ToolbarButton>
      </ToolbarSurface>
    </div>
  );
}
