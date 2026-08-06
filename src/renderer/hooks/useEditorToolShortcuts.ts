import { useEffect, useCallback } from 'react';
import type { ToolType } from '@/types/editor';
import type { EditorShortcuts } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';
import { shouldIgnoreGlobalKeyboardShortcuts } from '@/renderer/utils/keyboard';

interface UseEditorToolShortcutsProps {
  shortcuts: EditorShortcuts | undefined;
  onToolChange: (tool: ToolType) => void;
  isTextEditing?: boolean;
}

export const useEditorToolShortcuts = ({
  shortcuts,
  onToolChange,
  isTextEditing = false,
}: UseEditorToolShortcutsProps) => {
  const editorShortcuts = shortcuts ?? DEFAULT_SETTINGS.shortcuts.editor;

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (isTextEditing) return;

      if (e.metaKey || e.ctrlKey || e.altKey) return;

      if (shouldIgnoreGlobalKeyboardShortcuts(e.target)) {
        return;
      }

      const key = e.key.toLowerCase();

      const toolMap: Record<string, ToolType> = {
        [editorShortcuts.pen]: 'pen',
        [editorShortcuts.highlight]: 'highlight',
        [editorShortcuts.rectangle]: 'rectangle',
        [editorShortcuts.circle]: 'circle',
        [editorShortcuts.line]: 'line',
        [editorShortcuts.arrow]: 'arrow',
        [editorShortcuts.text]: 'text',
        [editorShortcuts.number]: 'number',
        [editorShortcuts.redact]: 'redact',
        [editorShortcuts.select]: 'select',
        [editorShortcuts.crop]: 'crop',
        [editorShortcuts.wallpaper]: 'wallpaper',
      };

      const tool = toolMap[key];
      if (tool) {
        e.preventDefault();
        onToolChange(tool);
      }
    },
    [editorShortcuts, onToolChange, isTextEditing]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);
};
