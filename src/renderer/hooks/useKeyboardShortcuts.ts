import { useEffect } from 'react';
import { shouldIgnoreGlobalKeyboardShortcuts } from '@/renderer/utils/keyboard';

interface UseKeyboardShortcutsProps {
  selectedAnnotationIds: string[];
  isTextEditing: boolean;
  onDeleteMultiple?: (ids: string[]) => void;
  onDeselect: () => void;
  annotationIds?: string[];
  onSelectAll?: (ids: string[]) => void;
}

export const useKeyboardShortcuts = ({
  selectedAnnotationIds,
  isTextEditing,
  onDeleteMultiple,
  onDeselect,
  annotationIds,
  onSelectAll,
}: UseKeyboardShortcutsProps) => {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (shouldIgnoreGlobalKeyboardShortcuts(e.target)) {
        return;
      }

      if (
        e.key === 'a' &&
        (e.metaKey || e.ctrlKey) &&
        !isTextEditing &&
        annotationIds &&
        annotationIds.length > 0 &&
        onSelectAll
      ) {
        e.preventDefault();
        onSelectAll(annotationIds);
        return;
      }

      if (
        (e.key === 'Delete' || e.key === 'Backspace') &&
        selectedAnnotationIds.length > 0 &&
        onDeleteMultiple &&
        !isTextEditing
      ) {
        e.preventDefault();
        onDeleteMultiple(selectedAnnotationIds);
        onDeselect();
      }

      if (e.key === 'Escape' && selectedAnnotationIds.length > 0) {
        e.preventDefault();
        onDeselect();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    selectedAnnotationIds,
    onDeleteMultiple,
    isTextEditing,
    onDeselect,
    annotationIds,
    onSelectAll,
  ]);
};
