import { useState, useCallback } from 'react';
import type { Annotation } from '@/types/editor';

const PASTE_OFFSET = 20;

export const useAnnotationClipboard = () => {
  const [clipboard, setClipboard] = useState<Annotation[]>([]);

  const copyAnnotations = useCallback((annotations: Annotation[]) => {
    setClipboard(structuredClone(annotations));
  }, []);

  const pasteAnnotations = useCallback((): Annotation[] => {
    return clipboard.map(ann => {
      const newId = crypto.randomUUID();
      const cloned = structuredClone(ann);
      cloned.id = newId;

      if ('x' in cloned && 'y' in cloned) {
        cloned.x += PASTE_OFFSET;
        cloned.y += PASTE_OFFSET;
      }

      if ('points' in cloned && Array.isArray(cloned.points)) {
        cloned.points = cloned.points.map(v => v + PASTE_OFFSET);
      }

      return cloned;
    });
  }, [clipboard]);

  return {
    copyAnnotations,
    pasteAnnotations,
    hasClipboard: clipboard.length > 0,
  };
};
