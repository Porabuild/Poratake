import {
  Camera,
  Download,
  Frame,
  Keyboard,
  MousePointer2,
  PenLine,
  Subtitles,
  Volume2,
  ZoomIn,
  Wallpaper,
} from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import {
  preloadEditorSidebarTab,
  type SidebarTab,
} from './editor-sidebar-panel-loaders';
import type { VideoEditorSidebarShortcuts } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';

interface TabItem {
  id: SidebarTab;
  label: string;
  icon: React.ReactNode;
}

interface EditorSidebarTabsProps {
  /** Null while the sidebar is closed, so no tab reads as selected. */
  activeTab: SidebarTab | null;
  onTabChange: (tab: SidebarTab) => void;
  shortcuts?: VideoEditorSidebarShortcuts;
}

export default function EditorSidebarTabs({
  activeTab,
  onTabChange,
  shortcuts,
}: EditorSidebarTabsProps) {
  const sidebarShortcuts =
    shortcuts ?? DEFAULT_SETTINGS.shortcuts.videoEditorSidebar;

  const getShortcutLabel = (tabId: SidebarTab): string => {
    const shortcut = sidebarShortcuts[tabId];
    return shortcut ? shortcut.toUpperCase() : '';
  };

  const tabs: TabItem[] = [
    {
      id: 'cursor',
      label: 'Cursor',
      icon: <MousePointer2 className="size-4" />,
    },
    {
      id: 'zoom',
      label: 'Zoom',
      icon: <ZoomIn className="size-4" />,
    },
    { id: 'drawing', label: 'Drawing', icon: <PenLine className="size-4" /> },
    { id: 'camera', label: 'Camera', icon: <Camera className="size-4" /> },
    { id: 'audio', label: 'Audio', icon: <Volume2 className="size-4" /> },
    {
      id: 'wallpaper',
      label: 'Wallpaper',
      icon: <Wallpaper className="size-4" />,
    },
    {
      id: 'keyboard',
      label: 'Keyboard',
      icon: <Keyboard className="size-4" />,
    },
    {
      id: 'subtitle',
      label: 'Subtitles',
      icon: <Subtitles className="size-4" />,
    },
    {
      id: 'first-frame',
      label: 'First Frame',
      icon: <Frame className="size-4" />,
    },
    { id: 'export', label: 'Export', icon: <Download className="size-4" /> },
  ];

  return (
    <div className="flex h-full w-10 shrink-0 flex-col items-center gap-1 border-l border-border bg-card py-2">
      {tabs.map(tab => {
        const shortcutLabel = getShortcutLabel(tab.id);
        return (
          <Tooltip key={tab.id}>
            <TooltipTrigger asChild>
              <Button
                variant={activeTab === tab.id ? 'tertiary' : 'ghost'}
                size="icon-sm"
                className="size-8!"
                onMouseEnter={() => preloadEditorSidebarTab(tab.id)}
                onFocus={() => preloadEditorSidebarTab(tab.id)}
                onClick={() => onTabChange(tab.id)}
              >
                {tab.icon}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="left" sideOffset={8}>
              {tab.label}
              {shortcutLabel && ` (${shortcutLabel})`}
            </TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
