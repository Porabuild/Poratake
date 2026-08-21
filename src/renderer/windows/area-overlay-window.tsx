import { useCallback, useEffect, useRef, useState } from 'react';
import AllInOneToolbar from '@/renderer/components/area-overlay/all-in-one-toolbar';
import ColorPicker from '@/renderer/components/area-overlay/color-picker';
import CrosshairGuides from '@/renderer/components/area-overlay/crosshair-guides';
import SelectionFrame from '@/renderer/components/area-overlay/selection-frame';
import SelectionScrim from '@/renderer/components/area-overlay/selection-scrim';
import useAreaSelection from '@/renderer/hooks/use-area-selection';
import type {
  AreaOverlayParams,
  AreaOverlayPickTargetsMessage,
  AreaOverlayRect,
  AreaOverlayToolbarAction,
  AreaOverlayToolbarMessage,
} from '@/types/area-overlay';
import { isMacPlatform } from '@/renderer/utils/platform';

function AreaOverlaySession({ params }: { params: AreaOverlayParams }) {
  const frozenFrame = useRef<HTMLImageElement>(null);
  const [toolbar, setToolbar] = useState(params.toolbar);
  const [prompt, setPrompt] = useState(params.prompt);
  const [isPickingColor, setIsPickingColor] = useState(false);
  const [handedOff, setHandedOff] = useState(false);
  const frameIdentity = `${params.displayId}:${params.imageUrl ?? ''}`;

  useEffect(() => {
    const handleHandoff = () => setHandedOff(true);

    return window.ipcRenderer.on('area-overlay:handoff', handleHandoff);
  }, []);

  const send = useCallback(
    (channel: string, rect: AreaOverlayRect, pickId?: number) => {
      window.ipcRenderer.send(channel, {
        displayId: params.displayId,
        pickId,
        ...rect,
      });
    },
    [params.displayId]
  );

  const cancel = useCallback(
    () => window.ipcRenderer.send('area-overlay:cancel'),
    []
  );

  const sendToolbarAction = useCallback(
    (action: AreaOverlayToolbarAction) =>
      window.ipcRenderer.send('area-overlay:toolbar', action),
    []
  );

  const startColorPick = useCallback(() => {
    window.ipcRenderer.send('area-overlay:color-picker', true);
  }, []);

  const stopColorPick = useCallback(() => {
    window.ipcRenderer.send('area-overlay:color-picker', false);
  }, []);

  const handleColorPicked = useCallback(
    (color: string) => {
      stopColorPick();
      sendToolbarAction({ action: 'copy-color', color });
    },
    [sendToolbarAction, stopColorPick]
  );

  useEffect(() => {
    const handleToolbar = (
      _event: unknown,
      message: AreaOverlayToolbarMessage
    ) => setToolbar(message.toolbar);

    const handlePickTargets = (
      _event: unknown,
      message: AreaOverlayPickTargetsMessage
    ) => setPrompt(message.prompt);

    const handleColorPicker = (_event: unknown, active: boolean) =>
      setIsPickingColor(active);

    const unsubscribeToolbar = window.ipcRenderer.on(
      'area-overlay:set-toolbar',
      handleToolbar
    );
    const unsubscribePickTargets = window.ipcRenderer.on(
      'area-overlay:set-pick-targets',
      handlePickTargets
    );
    const unsubscribeColorPicker = window.ipcRenderer.on(
      'area-overlay:set-color-picker',
      handleColorPicker
    );
    return () => {
      unsubscribeToolbar();
      unsubscribePickTargets();
      unsubscribeColorPicker();
    };
  }, []);

  const {
    rect,
    pointer,
    cursor,
    interacting,
    picking,
    locked,
    hovered,
    bounds,
    startDrag,
    trackPointer,
    clearPointer,
  } = useAreaSelection({
    interactive: params.interactive,
    initialRect: params.rect,
    initialAspectRatio: params.aspectRatio,
    pickTargets: params.pickTargets,
    repeatablePicks: params.repeatablePicks,
    onSelected: (selection, pickId) =>
      send(
        params.autoConfirm ? 'area-overlay:confirm' : 'area-overlay:selected',
        selection,
        pickId
      ),
    onUpdated: selection => send('area-overlay:updated', selection),
    onDiscarded: () => {
      if (params.interactive) {
        cancel();
      }
    },
  });

  useEffect(() => {
    (document.activeElement as HTMLElement | null)?.blur();
  }, []);

  useEffect(() => {
    const announceReady = () =>
      window.ipcRenderer.send('area-overlay:ready', params.sessionId);
    const image = frozenFrame.current;

    if (!image || image.dataset.frameIdentity !== frameIdentity) {
      announceReady();
      return;
    }

    image.decode().then(announceReady, announceReady);
  }, [frameIdentity, params.sessionId]);

  const revealedSessionIdRef = useRef(params.sessionId);

  useEffect(() => {
    revealedSessionIdRef.current = params.sessionId;
  }, [params.sessionId]);

  useEffect(() => {
    const handleRevealed = (_event: unknown, sessionId: number) => {
      if (sessionId !== revealedSessionIdRef.current) return;
      requestAnimationFrame(() => {
        window.ipcRenderer.send(
          'area-overlay:visible',
          revealedSessionIdRef.current
        );
      });
    };

    return window.ipcRenderer.on('area-overlay:revealed', handleRevealed);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isPickingColor) {
        cancel();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [cancel, isPickingColor]);

  const visibleRect = rect && rect.width > 0 && rect.height > 0 ? rect : null;
  const selectionScrimRect =
    handedOff && visibleRect
      ? {
          x: visibleRect.x - 1,
          y: visibleRect.y - 1,
          width: visibleRect.width + 2,
          height: visibleRect.height + 2,
        }
      : visibleRect;
  const scrimRect = picking
    ? (hovered ?? selectionScrimRect)
    : selectionScrimRect;

  return (
    <div
      className="fixed inset-0 overflow-hidden select-none"
      style={{ cursor: isPickingColor ? 'crosshair' : cursor }}
      onMouseDown={isPickingColor ? undefined : startDrag}
      onMouseMove={event =>
        !isPickingColor && trackPointer({ x: event.clientX, y: event.clientY })
      }
      onMouseLeave={clearPointer}
    >
      {params.imageUrl ? (
        <img
          ref={frozenFrame}
          src={params.imageUrl}
          data-frame-identity={frameIdentity}
          className="pointer-events-none absolute inset-0 h-full w-full"
          alt=""
          draggable={false}
        />
      ) : null}
      {!isPickingColor ? <SelectionScrim rect={scrimRect} /> : null}
      {visibleRect && !isPickingColor && !handedOff ? (
        <SelectionFrame
          rect={visibleRect}
          viewportHeight={bounds.height}
          interactive={params.interactive && !locked}
        />
      ) : null}
      {!picking &&
      !isPickingColor &&
      !interacting &&
      !visibleRect &&
      pointer ? (
        <CrosshairGuides x={pointer.x} y={pointer.y} />
      ) : null}
      {params.showPrompt &&
      !isPickingColor &&
      !interacting &&
      !visibleRect &&
      !handedOff ? (
        <div
          className={`pointer-events-none absolute left-1/2 -translate-x-1/2 rounded-full bg-black/70 px-4 py-2 text-sm text-white shadow-lg ${
            toolbar
              ? isMacPlatform()
                ? 'top-28'
                : 'top-24'
              : isMacPlatform()
                ? 'top-12'
                : 'top-8'
          }`}
        >
          {picking
            ? (prompt ?? 'Click to select · Esc to cancel')
            : 'Drag to select an area · Esc to cancel'}
        </div>
      ) : null}
      {toolbar && !handedOff && !isPickingColor ? (
        <AllInOneToolbar
          recordingEnabled={toolbar.recordingEnabled}
          ocrEnabled={toolbar.ocrEnabled}
          activeMode={toolbar.activeMode}
          activeTarget={toolbar.activeTarget}
          onAction={sendToolbarAction}
          onPickColor={startColorPick}
        />
      ) : null}
      {isPickingColor ? (
        <ColorPicker onPick={handleColorPicked} onCancel={stopColorPick} />
      ) : null}
    </div>
  );
}

export default function AreaOverlayWindow({
  params,
}: {
  params: AreaOverlayParams;
}) {
  return <AreaOverlaySession key={params.sessionId} params={params} />;
}
