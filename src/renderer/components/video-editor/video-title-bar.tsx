import {
  Trash2,
  PanelRightClose,
  PanelRightOpen,
  RefreshCcw,
} from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import UndoRedoButtons from '@/renderer/components/editor/undo-redo';
import WindowControlsSpacer from '@/renderer/components/window-controls-spacer';
import { isMacPlatform } from '@/renderer/utils/platform';
import { formatAccelerator } from '@/renderer/utils/shortcuts';
import { cn } from '@/renderer/lib/utils';
import ExportProgressIndicator from './export-progress-indicator';
import ProjectPathIndicator from './project-path-indicator';

interface VideoTitleBarProps {
  fileName?: string;
  projectPath?: string;
  onDelete: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onReset: () => void;
  canUndo: boolean;
  canRedo: boolean;
  isSidebarOpen?: boolean;
  onToggleSidebar?: () => void;
  isExporting?: boolean;
  exportProgress?: number;
  onCancelExport?: () => void;
  onRename?: (newName: string) => Promise<string | null>;
}

export default function VideoTitleBar({
  fileName,
  projectPath,
  onDelete,
  onUndo,
  onRedo,
  onReset,
  canUndo,
  canRedo,
  isSidebarOpen = false,
  onToggleSidebar,
  isExporting = false,
  exportProgress = 0,
  onCancelExport,
  onRename,
}: VideoTitleBarProps) {
  return (
    <div className="drag-region fixed top-0 right-0 left-0 z-50 flex h-10 w-full items-center justify-between bg-card px-2">
      <div
        className={cn(
          'flex min-w-0 flex-1 items-center',
          isMacPlatform() && 'pl-20'
        )}
      >
        <span className="truncate text-sm text-muted-foreground">
          {fileName || 'Untitled'}
        </span>
      </div>
      <div className="flex shrink-0 items-center justify-end gap-1">
        {projectPath && (
          <ProjectPathIndicator
            projectPath={projectPath}
            fileName={fileName}
            onRename={onRename}
          />
        )}
        {onCancelExport && (
          <ExportProgressIndicator
            isExporting={isExporting}
            progress={exportProgress}
            onCancel={onCancelExport}
          />
        )}
        <UndoRedoButtons
          onUndo={onUndo}
          onRedo={onRedo}
          canUndo={canUndo}
          canRedo={canRedo}
        />
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              onClick={onReset}
              variant="ghost"
              size="icon-sm"
              className="size-7!"
            >
              <RefreshCcw className="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">Reset to Defaults</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              onClick={onDelete}
              variant="ghost"
              size="icon-sm"
              className="size-7!"
            >
              <Trash2 className="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            Delete Video ({formatAccelerator('CommandOrControl+Backspace')})
          </TooltipContent>
        </Tooltip>

        {onToggleSidebar && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                onClick={onToggleSidebar}
                variant="ghost"
                size="icon-sm"
                className="size-7!"
              >
                {isSidebarOpen ? (
                  <PanelRightClose className="size-4" />
                ) : (
                  <PanelRightOpen className="size-4" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              {isSidebarOpen ? 'Hide Sidebar' : 'Show Sidebar'}
            </TooltipContent>
          </Tooltip>
        )}
      </div>
      <WindowControlsSpacer />
    </div>
  );
}
