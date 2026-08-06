import { useCallback } from 'react';

export function useStyleUpdater<T>(
  style: T,
  onStyleChange: (style: T) => void
): (updates: Partial<T>) => void {
  return useCallback(
    (updates: Partial<T>) => {
      onStyleChange({ ...style, ...updates });
    },
    [style, onStyleChange]
  );
}
