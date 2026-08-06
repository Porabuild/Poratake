import { useState, useCallback } from 'react';

export const useAnnotationSelection = () => {
  const [selectedAnnotationIds, setSelectedAnnotationIds] = useState<string[]>(
    []
  );

  const selectAnnotation = useCallback(
    (id: string | null, addToSelection = false) => {
      if (id === null) {
        setSelectedAnnotationIds([]);
        return;
      }

      if (addToSelection) {
        setSelectedAnnotationIds(prev => {
          if (prev.includes(id)) {
            return prev.filter(selectedId => selectedId !== id);
          }
          return [...prev, id];
        });
      } else {
        setSelectedAnnotationIds([id]);
      }
    },
    []
  );

  const selectMultiple = useCallback((ids: string[]) => {
    setSelectedAnnotationIds(ids);
  }, []);

  const deselectAll = useCallback(() => {
    setSelectedAnnotationIds([]);
  }, []);

  const isSelected = useCallback(
    (id: string) => selectedAnnotationIds.includes(id),
    [selectedAnnotationIds]
  );

  return {
    selectedAnnotationIds,
    selectAnnotation,
    selectMultiple,
    deselectAll,
    isSelected,
  };
};
