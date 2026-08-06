import { useCallback, useEffect, useRef, useState } from 'react';

interface UseResizableHeightProps {
  storageKey: string;
  defaultHeight: number;
  minHeight: number;
  maxHeight: number;
}

interface UseResizableHeightReturn {
  height: number;
  isResizing: boolean;
  startResize: (event: React.MouseEvent) => void;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function readStoredHeight(storageKey: string, fallback: number): number {
  const stored = window.localStorage.getItem(storageKey);
  if (stored === null) return fallback;
  const parsed = Number.parseFloat(stored);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function useResizableHeight({
  storageKey,
  defaultHeight,
  minHeight,
  maxHeight,
}: UseResizableHeightProps): UseResizableHeightReturn {
  const [height, setHeight] = useState(() =>
    clamp(readStoredHeight(storageKey, defaultHeight), minHeight, maxHeight)
  );
  const [isResizing, setIsResizing] = useState(false);
  const dragRef = useRef<{ startY: number; startHeight: number } | null>(null);

  useEffect(() => {
    setHeight(prev => clamp(prev, minHeight, maxHeight));
  }, [minHeight, maxHeight]);

  useEffect(() => {
    window.localStorage.setItem(storageKey, String(height));
  }, [storageKey, height]);

  const heightRef = useRef(height);
  heightRef.current = height;

  const startResize = useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    dragRef.current = {
      startY: event.clientY,
      startHeight: heightRef.current,
    };
    setIsResizing(true);
  }, []);

  useEffect(() => {
    if (!isResizing) return;

    const handleMouseMove = (event: MouseEvent) => {
      const drag = dragRef.current;
      if (!drag) return;
      const delta = drag.startY - event.clientY;
      setHeight(clamp(drag.startHeight + delta, minHeight, maxHeight));
    };

    const handleMouseUp = () => {
      dragRef.current = null;
      setIsResizing(false);
    };

    const previousUserSelect = document.body.style.userSelect;
    const previousCursor = document.body.style.cursor;
    document.body.style.userSelect = 'none';
    document.body.style.cursor = 'ns-resize';

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
      document.body.style.userSelect = previousUserSelect;
      document.body.style.cursor = previousCursor;
    };
  }, [isResizing, minHeight, maxHeight]);

  return { height, isResizing, startResize };
}
