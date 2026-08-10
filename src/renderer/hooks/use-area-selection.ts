import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import {
  adjustRectToRatio,
  clampPoint,
  containsPoint,
  cursorFor,
  fitRect,
  hitTestHandle,
  isUsableSelection,
  moveRect,
  normalizeRect,
  resizeRect,
} from '@/renderer/utils/area-selection';
import type {
  Bounds,
  Point,
  SelectionHandle,
} from '@/renderer/utils/area-selection';
import type {
  AreaOverlayAspectRatioMessage,
  AreaOverlayRect,
  AreaOverlayRectMessage,
} from '@/types/area-overlay';

type Interaction =
  | { type: 'creating'; start: Point }
  | { type: 'moving'; offset: Point }
  | { type: 'resizing'; handle: SelectionHandle };

export interface AreaSelectionOptions {
  resetKey: number;
  interactive: boolean;
  initialRect: AreaOverlayRect | null;
  initialAspectRatio: number | null;
  onSelected: (rect: AreaOverlayRect) => void;
  onUpdated: (rect: AreaOverlayRect) => void;
  onDiscarded: () => void;
}

export default function useAreaSelection(options: AreaSelectionOptions) {
  const optionsRef = useRef(options);
  optionsRef.current = options;

  const [bounds, setBounds] = useState<Bounds>(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
  }));
  const [rect, setRect] = useState<AreaOverlayRect | null>(options.initialRect);
  const [pointer, setPointer] = useState<Point | null>(null);
  const [cursor, setCursor] = useState('crosshair');
  const [interacting, setInteracting] = useState(false);

  const rectRef = useRef(options.initialRect);
  const ratioRef = useRef(options.initialAspectRatio);
  const interactionRef = useRef<Interaction | null>(null);

  const applyRect = useCallback((next: AreaOverlayRect | null) => {
    rectRef.current = next;
    setRect(next);
  }, []);

  useLayoutEffect(() => {
    ratioRef.current = options.initialAspectRatio;
    interactionRef.current = null;
    setBounds({ width: window.innerWidth, height: window.innerHeight });
    applyRect(options.initialRect);
    setPointer(null);
    setCursor('crosshair');
    setInteracting(false);
  }, [
    applyRect,
    options.initialAspectRatio,
    options.initialRect,
    options.resetKey,
  ]);

  useEffect(() => {
    const handleRect = (_event: unknown, message: AreaOverlayRectMessage) => {
      applyRect(message.rect);
    };

    const handleAspectRatio = (
      _event: unknown,
      message: AreaOverlayAspectRatioMessage
    ) => {
      ratioRef.current = message.aspectRatio;

      const current = rectRef.current;
      if (!current || !message.aspectRatio) return;

      const next = fitRect(
        adjustRectToRatio(current, message.aspectRatio, null),
        bounds
      );
      applyRect(next);
      optionsRef.current.onUpdated(next);
    };

    window.ipcRenderer.on('area-overlay:set-rect', handleRect);
    window.ipcRenderer.on('area-overlay:set-aspect-ratio', handleAspectRatio);

    return () => {
      window.ipcRenderer.off('area-overlay:set-rect', handleRect);
      window.ipcRenderer.off(
        'area-overlay:set-aspect-ratio',
        handleAspectRatio
      );
    };
  }, [applyRect, bounds]);

  const trackPointer = useCallback((point: Point) => {
    setPointer(point);
    setCursor(cursorFor(rectRef.current, point));
  }, []);

  const dragTo = useCallback(
    (point: Point) => {
      const interaction = interactionRef.current;
      if (!interaction) return;

      const ratio = ratioRef.current;

      if (interaction.type === 'creating') {
        const created = normalizeRect(interaction.start, point);
        applyRect(
          ratio
            ? fitRect(adjustRectToRatio(created, ratio, null), bounds)
            : created
        );
        return;
      }

      const current = rectRef.current;
      if (!current) return;

      const next =
        interaction.type === 'moving'
          ? moveRect(current, point, interaction.offset, bounds)
          : fitRect(
              resizeRect(current, point, interaction.handle, ratio),
              bounds
            );

      applyRect(next);
      optionsRef.current.onUpdated(next);
    },
    [applyRect, bounds]
  );

  const endDrag = useCallback(
    (point: Point) => {
      const interaction = interactionRef.current;
      interactionRef.current = null;
      setInteracting(false);

      const current = rectRef.current;
      setCursor(cursorFor(current, point));

      if (!interaction || !current) return;

      if (interaction.type !== 'creating') {
        optionsRef.current.onUpdated(current);
        return;
      }

      if (!isUsableSelection(current)) {
        applyRect(null);
        optionsRef.current.onDiscarded();
        return;
      }

      optionsRef.current.onSelected(current);
    },
    [applyRect]
  );

  useEffect(() => {
    if (!interacting) return;

    const toPoint = (event: MouseEvent) =>
      clampPoint({ x: event.clientX, y: event.clientY }, bounds);

    const handleMove = (event: MouseEvent) => dragTo(toPoint(event));
    const handleUp = (event: MouseEvent) => endDrag(toPoint(event));

    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', handleUp);

    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', handleUp);
    };
  }, [bounds, dragTo, endDrag, interacting]);

  const startDrag = useCallback(
    (event: React.MouseEvent) => {
      if (event.button !== 0) return;

      const point = clampPoint({ x: event.clientX, y: event.clientY }, bounds);
      const current = rectRef.current;

      if (optionsRef.current.interactive && current) {
        const handle = hitTestHandle(current, point);

        if (handle) {
          interactionRef.current = { type: 'resizing', handle };
        } else if (containsPoint(current, point)) {
          interactionRef.current = {
            type: 'moving',
            offset: { x: point.x - current.x, y: point.y - current.y },
          };
        }
      }

      if (!interactionRef.current) {
        interactionRef.current = { type: 'creating', start: point };
        applyRect({ x: point.x, y: point.y, width: 0, height: 0 });
      }

      setInteracting(true);
    },
    [applyRect, bounds]
  );

  const clearPointer = useCallback(() => setPointer(null), []);

  return {
    rect,
    pointer,
    cursor,
    interacting,
    bounds,
    startDrag,
    trackPointer,
    clearPointer,
  };
}
