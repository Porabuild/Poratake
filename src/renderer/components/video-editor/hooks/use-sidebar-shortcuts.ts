import { useEffect, useCallback } from 'react';
import type { SidebarTab } from '../editor-sidebar';
import type { VideoEditorSidebarShortcuts } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';
import { shouldIgnoreGlobalKeyboardShortcuts } from '@/renderer/utils/keyboard';

interface UseSidebarShortcutsProps {
  shortcuts: VideoEditorSidebarShortcuts | undefined;
  onTabChange: (tab: SidebarTab) => void;
}

export function useSidebarShortcuts({
  shortcuts,
  onTabChange,
}: UseSidebarShortcutsProps): void {
  const sidebarShortcuts =
    shortcuts ?? DEFAULT_SETTINGS.shortcuts.videoEditorSidebar;

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      if (shouldIgnoreGlobalKeyboardShortcuts(e.target)) {
        return;
      }

      const key = e.key.toLowerCase();

      const tabMap: Record<string, SidebarTab> = {
        [sidebarShortcuts.cursor]: 'cursor',
        [sidebarShortcuts.zoom]: 'zoom',
        [sidebarShortcuts.drawing ?? '']: 'drawing',
        [sidebarShortcuts.camera]: 'camera',
        [sidebarShortcuts.audio]: 'audio',
        [sidebarShortcuts.wallpaper]: 'wallpaper',
        [sidebarShortcuts.keyboard]: 'keyboard',
        [sidebarShortcuts.subtitle]: 'subtitle',
        [sidebarShortcuts['first-frame']]: 'first-frame',
        [sidebarShortcuts.export]: 'export',
      };

      const tab = tabMap[key];
      if (tab) {
        e.preventDefault();
        onTabChange(tab);
      }
    },
    [sidebarShortcuts, onTabChange]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);
}
