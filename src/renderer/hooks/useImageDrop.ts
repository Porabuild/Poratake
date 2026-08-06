import { useState, useCallback, useRef, useEffect } from 'react';

export type DropEdge = 'top' | 'bottom' | 'left' | 'right' | null;

interface UseImageDropProps {
  onImageDrop: (imageBase64: string, edge: DropEdge) => void;
  dropTargetRef: React.RefObject<HTMLElement | null>;
  enabled?: boolean;
}

interface UseImageDropReturn {
  isDragging: boolean;
  dropEdge: DropEdge;
}

function getDropEdge(
  clientX: number,
  clientY: number,
  rect: DOMRect
): DropEdge {
  const relX = (clientX - rect.left) / rect.width;
  const relY = (clientY - rect.top) / rect.height;

  const distTop = relY;
  const distBottom = 1 - relY;
  const distLeft = relX;
  const distRight = 1 - relX;

  const min = Math.min(distTop, distBottom, distLeft, distRight);

  if (min === distTop) return 'top';
  if (min === distBottom) return 'bottom';
  if (min === distLeft) return 'left';
  return 'right';
}

function hasImageFile(dataTransfer: DataTransfer): boolean {
  for (const item of Array.from(dataTransfer.items)) {
    if (item.kind === 'file' && item.type.startsWith('image/')) {
      return true;
    }
  }
  return false;
}

function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      const base64 = result.split(',')[1];
      resolve(base64);
    };
    reader.onerror = () => reject(new Error('Failed to read file'));
    reader.readAsDataURL(file);
  });
}

export function useImageDrop({
  onImageDrop,
  dropTargetRef,
  enabled = true,
}: UseImageDropProps): UseImageDropReturn {
  const [isDragging, setIsDragging] = useState(false);
  const [dropEdge, setDropEdge] = useState<DropEdge>(null);
  const dragCounterRef = useRef(0);

  const handleDragEnter = useCallback(
    (e: DragEvent) => {
      if (!enabled) return;
      if (!e.dataTransfer || !hasImageFile(e.dataTransfer)) return;

      e.preventDefault();
      e.stopPropagation();
      dragCounterRef.current++;
      setIsDragging(true);
    },
    [enabled]
  );

  const handleDragOver = useCallback(
    (e: DragEvent) => {
      if (!enabled || !isDragging) return;
      if (!e.dataTransfer) return;

      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = 'copy';

      const target = dropTargetRef.current;
      if (!target) return;

      const rect = target.getBoundingClientRect();
      setDropEdge(getDropEdge(e.clientX, e.clientY, rect));
    },
    [enabled, isDragging, dropTargetRef]
  );

  const handleDragLeave = useCallback(
    (e: DragEvent) => {
      if (!enabled) return;

      e.preventDefault();
      e.stopPropagation();
      dragCounterRef.current--;

      if (dragCounterRef.current <= 0) {
        dragCounterRef.current = 0;
        setIsDragging(false);
        setDropEdge(null);
      }
    },
    [enabled]
  );

  const handleDrop = useCallback(
    async (e: DragEvent) => {
      if (!enabled) return;

      e.preventDefault();
      e.stopPropagation();
      dragCounterRef.current = 0;
      setIsDragging(false);

      const currentEdge = dropEdge;
      setDropEdge(null);

      if (!e.dataTransfer || !currentEdge) return;

      const files = Array.from(e.dataTransfer.files).filter(f =>
        f.type.startsWith('image/')
      );

      if (files.length === 0) return;

      try {
        const base64 = await readFileAsBase64(files[0]);
        onImageDrop(base64, currentEdge);
      } catch (error) {
        console.error('Failed to read dropped image:', error);
      }
    },
    [enabled, dropEdge, onImageDrop]
  );

  useEffect(() => {
    const target = dropTargetRef.current;
    if (!target || !enabled) return;

    target.addEventListener('dragenter', handleDragEnter);
    target.addEventListener('dragover', handleDragOver);
    target.addEventListener('dragleave', handleDragLeave);
    target.addEventListener('drop', handleDrop);

    return () => {
      target.removeEventListener('dragenter', handleDragEnter);
      target.removeEventListener('dragover', handleDragOver);
      target.removeEventListener('dragleave', handleDragLeave);
      target.removeEventListener('drop', handleDrop);
    };
  }, [
    dropTargetRef,
    enabled,
    handleDragEnter,
    handleDragOver,
    handleDragLeave,
    handleDrop,
  ]);

  return { isDragging, dropEdge };
}
