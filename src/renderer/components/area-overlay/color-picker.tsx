import { useEffect, useRef, useState } from 'react';

const GRID = 15;
const CELL = 7;
const SIZE = GRID * CELL;
const HALF = Math.floor(GRID / 2);
const CURSOR_OFFSET = 20;

interface ColorFrame {
  url: string;
}

export interface SampleSource {
  data: Uint8ClampedArray;
  width: number;
  height: number;
}

function toHex(r: number, g: number, b: number): string {
  return (
    '#' + [r, g, b].map(value => value.toString(16).padStart(2, '0')).join('')
  );
}

export function sampleHex(source: SampleSource, x: number, y: number): string {
  const index = (y * source.width + x) * 4;
  return toHex(
    source.data[index],
    source.data[index + 1],
    source.data[index + 2]
  );
}

export function fillLoupePixels(
  source: SampleSource,
  x: number,
  y: number,
  target: Uint8ClampedArray
): void {
  for (let row = 0; row < GRID; row += 1) {
    const sourceY = Math.min(source.height - 1, Math.max(0, y - HALF + row));
    for (let column = 0; column < GRID; column += 1) {
      const sourceX = Math.min(
        source.width - 1,
        Math.max(0, x - HALF + column)
      );
      const sourceIndex = (sourceY * source.width + sourceX) * 4;
      const targetIndex = (row * GRID + column) * 4;
      target[targetIndex] = source.data[sourceIndex];
      target[targetIndex + 1] = source.data[sourceIndex + 1];
      target[targetIndex + 2] = source.data[sourceIndex + 2];
      target[targetIndex + 3] = source.data[sourceIndex + 3];
    }
  }
}

export default function ColorPicker({
  onPick,
  onCancel,
}: {
  onPick: (color: string) => void;
  onCancel: () => void;
}) {
  const [color, setColor] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const sourceRef = useRef<SampleSource | null>(null);
  const pointerRef = useRef<{ x: number; y: number } | null>(null);
  const lastColorRef = useRef<string | null>(null);
  const renderAtRef = useRef<((x: number, y: number) => void) | null>(null);
  const rafRef = useRef<number | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const loupeRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let disposed = false;

    (async () => {
      const frame = (await window.ipcRenderer.invoke(
        'area-overlay:color-picker-frame'
      )) as ColorFrame | null;
      if (disposed) return;
      if (!frame) {
        onCancel();
        return;
      }

      const image = new Image();
      image.src = frame.url;
      try {
        await image.decode();
      } catch {
        if (!disposed) onCancel();
        return;
      }
      if (disposed) return;

      const canvas = document.createElement('canvas');
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      if (!context) {
        onCancel();
        return;
      }

      context.drawImage(image, 0, 0);
      sourceRef.current = {
        data: context.getImageData(0, 0, canvas.width, canvas.height).data,
        width: canvas.width,
        height: canvas.height,
      };
      canvas.width = 0;
      canvas.height = 0;
      setReady(true);
    })();

    return () => {
      disposed = true;
      sourceRef.current = null;
    };
  }, [onCancel]);

  useEffect(() => {
    const handleMove = (event: MouseEvent) => {
      pointerRef.current = { x: event.clientX, y: event.clientY };
      if (rafRef.current !== null) return;

      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = null;
        const pointer = pointerRef.current;
        if (pointer) {
          renderAtRef.current?.(pointer.x, pointer.y);
        }
      });
    };

    window.addEventListener('mousemove', handleMove);
    return () => {
      window.removeEventListener('mousemove', handleMove);
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    const handleMouseDown = (event: MouseEvent) => {
      if (event.button !== 0) return;
      renderAtRef.current?.(event.clientX, event.clientY);
      const current = lastColorRef.current;
      if (current) onPick(current);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onCancel();
    };

    window.addEventListener('mousedown', handleMouseDown);
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('mousedown', handleMouseDown);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [onPick, onCancel]);

  useEffect(() => {
    if (!ready) return;

    const dpr = window.devicePixelRatio || 1;
    const loupe = loupeRef.current;
    const loupeContext = loupe?.getContext('2d') ?? null;
    if (loupe) {
      loupe.width = Math.round(SIZE * dpr);
      loupe.height = Math.round(SIZE * dpr);
    }
    if (loupeContext) {
      loupeContext.imageSmoothingEnabled = false;
    }

    const sampleCanvas = document.createElement('canvas');
    sampleCanvas.width = GRID;
    sampleCanvas.height = GRID;
    const sampleContext = sampleCanvas.getContext('2d');
    const sampleData = sampleContext?.createImageData(GRID, GRID) ?? null;

    const measure = cardRef.current?.getBoundingClientRect();
    const cardSize = {
      width: measure?.width ?? SIZE,
      height: measure?.height ?? SIZE,
    };

    const renderAt = (x: number, y: number) => {
      const source = sourceRef.current;
      const card = cardRef.current;
      if (
        !source ||
        !card ||
        !loupe ||
        !loupeContext ||
        !sampleContext ||
        !sampleData
      ) {
        return;
      }

      const scaleX = source.width / window.innerWidth;
      const scaleY = source.height / window.innerHeight;
      const sx = Math.min(
        source.width - 1,
        Math.max(0, Math.round(x * scaleX))
      );
      const sy = Math.min(
        source.height - 1,
        Math.max(0, Math.round(y * scaleY))
      );

      const hex = sampleHex(source, sx, sy);
      if (hex !== lastColorRef.current) {
        lastColorRef.current = hex;
        setColor(hex);
      }

      fillLoupePixels(source, sx, sy, sampleData.data);
      sampleContext.putImageData(sampleData, 0, 0);
      loupeContext.clearRect(0, 0, loupe.width, loupe.height);
      loupeContext.drawImage(
        sampleCanvas,
        0,
        0,
        GRID,
        GRID,
        0,
        0,
        loupe.width,
        loupe.height
      );

      let cardX = x + CURSOR_OFFSET;
      let cardY = y + CURSOR_OFFSET;
      if (cardX + cardSize.width > window.innerWidth) {
        cardX = x - CURSOR_OFFSET - cardSize.width;
      }
      if (cardY + cardSize.height > window.innerHeight) {
        cardY = y - CURSOR_OFFSET - cardSize.height;
      }
      card.style.transform = `translate3d(${cardX}px, ${cardY}px, 0)`;
    };

    renderAtRef.current = renderAt;
    const pointer = pointerRef.current;
    if (pointer) {
      renderAt(pointer.x, pointer.y);
    }
    return () => {
      renderAtRef.current = null;
      sampleCanvas.width = 0;
      sampleCanvas.height = 0;
    };
  }, [ready]);

  if (!ready) {
    return null;
  }

  return (
    <div
      ref={cardRef}
      className="pointer-events-none absolute top-0 left-0 will-change-transform"
    >
      <div className="rounded-2xl border-2 border-muted-foreground/35 bg-muted/95 p-1.5 shadow-2xl backdrop-blur-xl">
        <div className="relative overflow-hidden rounded-lg">
          <canvas ref={loupeRef} style={{ width: SIZE, height: SIZE }} />
          <div
            className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 border border-white/90 outline-1 outline-black/40"
            style={{ width: CELL, height: CELL }}
          />
        </div>
        <div className="mt-1.5 flex items-center gap-1.5 px-0.5">
          <span
            className="size-3.5 shrink-0 rounded-full border border-border/60"
            style={{ backgroundColor: color ?? 'transparent' }}
          />
          <span className="font-mono text-xs font-medium text-foreground">
            {color?.toUpperCase() ?? '· · ·'}
          </span>
        </div>
        <div className="mt-1 px-0.5 text-xs text-muted-foreground">
          Click to copy · Esc to cancel
        </div>
      </div>
    </div>
  );
}
