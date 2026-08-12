import { useCallback, useEffect, useRef, useState } from 'react';

type ResizeAxis = 'vertical' | 'horizontal';

interface UseResizablePaneProps {
  storageKey: string;
  /** `vertical` resizes height (drag up to grow), `horizontal` resizes width (drag left to grow). */
  axis: ResizeAxis;
  defaultSize: number;
  minSize: number;
  maxSize: number;
}

interface UseResizablePaneReturn {
  size: number;
  isResizing: boolean;
  startResize: (event: React.MouseEvent) => void;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function readStoredSize(storageKey: string, fallback: number): number {
  const stored = window.localStorage.getItem(storageKey);
  if (stored === null) return fallback;
  const parsed = Number.parseFloat(stored);
  return Number.isFinite(parsed) ? parsed : fallback;
}

/**
 * Drag-to-resize for panes anchored to the bottom or right edge of the editor:
 * the pane grows as the pointer moves towards the start of the axis.
 */
export function useResizablePane({
  storageKey,
  axis,
  defaultSize,
  minSize,
  maxSize,
}: UseResizablePaneProps): UseResizablePaneReturn {
  const [size, setSize] = useState(() =>
    clamp(readStoredSize(storageKey, defaultSize), minSize, maxSize)
  );
  const [isResizing, setIsResizing] = useState(false);
  const dragRef = useRef<{ start: number; startSize: number } | null>(null);

  useEffect(() => {
    setSize(prev => clamp(prev, minSize, maxSize));
  }, [minSize, maxSize]);

  useEffect(() => {
    window.localStorage.setItem(storageKey, String(size));
  }, [storageKey, size]);

  const sizeRef = useRef(size);
  sizeRef.current = size;

  const startResize = useCallback(
    (event: React.MouseEvent) => {
      event.preventDefault();
      dragRef.current = {
        start: axis === 'vertical' ? event.clientY : event.clientX,
        startSize: sizeRef.current,
      };
      setIsResizing(true);
    },
    [axis]
  );

  useEffect(() => {
    if (!isResizing) return;

    const handleMouseMove = (event: MouseEvent) => {
      const drag = dragRef.current;
      if (!drag) return;
      const position = axis === 'vertical' ? event.clientY : event.clientX;
      const delta = drag.start - position;
      setSize(clamp(drag.startSize + delta, minSize, maxSize));
    };

    const handleMouseUp = () => {
      dragRef.current = null;
      setIsResizing(false);
    };

    const previousUserSelect = document.body.style.userSelect;
    const previousCursor = document.body.style.cursor;
    document.body.style.userSelect = 'none';
    document.body.style.cursor =
      axis === 'vertical' ? 'ns-resize' : 'ew-resize';

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
      document.body.style.userSelect = previousUserSelect;
      document.body.style.cursor = previousCursor;
    };
  }, [isResizing, axis, minSize, maxSize]);

  return { size, isResizing, startResize };
}
