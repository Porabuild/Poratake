import { useCallback, useEffect, useRef, useState } from 'react';

interface PanOrigin {
  pointerX: number;
  pointerY: number;
  scrollLeft: number;
  scrollTop: number;
}

export interface PanOnDrag {
  isPanning: boolean;
  onMouseDownCapture: (event: React.MouseEvent) => void;
}

export function usePanOnDrag(
  containerRef: React.RefObject<HTMLElement | null>,
  enabled: boolean = true
): PanOnDrag {
  const [isPanning, setIsPanning] = useState(false);
  const originRef = useRef<PanOrigin>({
    pointerX: 0,
    pointerY: 0,
    scrollLeft: 0,
    scrollTop: 0,
  });

  const onMouseDownCapture = useCallback(
    (event: React.MouseEvent) => {
      const container = containerRef.current;
      if (!enabled || !container) return;
      if (event.button !== 0 || !(event.ctrlKey || event.metaKey)) return;
      if (
        (event.target as Element).closest('button, input, select, textarea')
      ) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      originRef.current = {
        pointerX: event.clientX,
        pointerY: event.clientY,
        scrollLeft: container.scrollLeft,
        scrollTop: container.scrollTop,
      };
      setIsPanning(true);
    },
    [containerRef, enabled]
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!isPanning || !container) return;

    const handleMove = (event: MouseEvent) => {
      const origin = originRef.current;
      container.scrollLeft =
        origin.scrollLeft - (event.clientX - origin.pointerX);
      container.scrollTop =
        origin.scrollTop - (event.clientY - origin.pointerY);
    };

    const stopPanning = () => setIsPanning(false);

    window.addEventListener('mousemove', handleMove);
    window.addEventListener('mouseup', stopPanning);
    window.addEventListener('blur', stopPanning);

    return () => {
      window.removeEventListener('mousemove', handleMove);
      window.removeEventListener('mouseup', stopPanning);
      window.removeEventListener('blur', stopPanning);
    };
  }, [isPanning, containerRef]);

  return { isPanning, onMouseDownCapture };
}
