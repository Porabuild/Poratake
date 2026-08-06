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
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import { cn } from '@/renderer/lib/utils';
import type { SidebarTab } from './editor-sidebar';
import type { VideoEditorSidebarShortcuts } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';

interface TabItem {
  id: SidebarTab;
  label: string;
  icon: React.ReactNode;
  disabled?: boolean;
}

interface EditorSidebarTabsProps {
  activeTab: SidebarTab;
  onTabChange: (tab: SidebarTab) => void;
  isZoomDisabled: boolean;
  shortcuts?: VideoEditorSidebarShortcuts;
}

export default function EditorSidebarTabs({
  activeTab,
  onTabChange,
  isZoomDisabled,
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
      disabled: isZoomDisabled,
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
    <div className="bg-card border-border flex h-full w-10 shrink-0 flex-col items-center gap-1 border-l py-2">
      {tabs.map(tab => {
        const shortcutLabel = getShortcutLabel(tab.id);
        return (
          <Tooltip key={tab.id}>
            <TooltipTrigger asChild>
              <button
                onClick={() => !tab.disabled && onTabChange(tab.id)}
                disabled={tab.disabled}
                className={cn(
                  'flex size-8 items-center justify-center rounded-md transition-colors',
                  activeTab === tab.id
                    ? 'bg-accent text-accent-foreground'
                    : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground',
                  tab.disabled && 'cursor-not-allowed opacity-50'
                )}
              >
                {tab.icon}
              </button>
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
