import { useEffect, useRef, useState, useCallback } from 'react';
import { FolderOpen, Copy, Check } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { Input } from '@/renderer/components/ui/input';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/renderer/components/ui/tooltip';
import { cn } from '@/renderer/lib/utils';

interface ProjectPathIndicatorProps {
  projectPath: string;
  fileName?: string;
  onRename?: (newName: string) => Promise<string | null>;
}

const COPY_FEEDBACK_DURATION_MS = 2000;

export default function ProjectPathIndicator({
  projectPath,
  fileName,
  onRename,
}: ProjectPathIndicatorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isCopied, setIsCopied] = useState(false);
  const [editName, setEditName] = useState(fileName ?? '');
  const [previousFileName, setPreviousFileName] = useState(fileName);
  if (previousFileName !== fileName) {
    setPreviousFileName(fileName);
    setEditName(fileName ?? '');
  }
  const [renameError, setRenameError] = useState<string | null>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);

  const popoverRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        popoverRef.current?.contains(target) ||
        triggerRef.current?.contains(target)
      ) {
        return;
      }
      setIsOpen(false);
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    nameInputRef.current?.select();
  }, [isOpen]);

  const handleToggle = useCallback(() => {
    setIsOpen(prev => !prev);
  }, []);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(projectPath);
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), COPY_FEEDBACK_DURATION_MS);
    } catch {
      console.error('Failed to copy path to clipboard');
    }
  }, [projectPath]);

  const handleOpenInFinder = useCallback(() => {
    window.ipcRenderer.send('shell:reveal-in-finder');
  }, []);

  const handleSaveRename = useCallback(async () => {
    const trimmed = editName.trim();
    if (!trimmed || trimmed === fileName) return;
    setRenameError(null);
    const error = await onRename?.(trimmed);
    if (error) {
      setRenameError(error);
    }
  }, [editName, fileName, onRename]);

  const handleNameKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        handleSaveRename();
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        setEditName(fileName ?? '');
        setIsOpen(false);
      }
    },
    [handleSaveRename, fileName]
  );

  const isNameChanged =
    editName.trim().length > 0 && editName.trim() !== fileName;

  if (!projectPath) return null;

  return (
    <div className="no-drag relative flex items-center">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            ref={triggerRef}
            variant={isOpen ? 'tertiary' : 'ghost'}
            size="icon-xs"
            className="size-7!"
            onClick={handleToggle}
          >
            <FolderOpen className="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom">Project Info</TooltipContent>
      </Tooltip>

      {isOpen && (
        <div
          ref={popoverRef}
          className={cn(
            'absolute top-full right-0 z-50 mt-1.5 w-80 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg',
            'animate-in fade-in-0 zoom-in-95 data-[side=bottom]:slide-in-from-top-2'
          )}
        >
          <div className="space-y-3 p-3">
            {onRename && (
              <>
                <span className="text-sm font-medium">Project Name</span>
                <div className="flex gap-2">
                  <Input
                    ref={nameInputRef}
                    value={editName}
                    onChange={e => {
                      setEditName(e.target.value);
                      setRenameError(null);
                    }}
                    onKeyDown={handleNameKeyDown}
                    className="h-8 flex-1 text-xs"
                    spellCheck={false}
                  />
                  <Button
                    variant="default"
                    size="xs"
                    onClick={handleSaveRename}
                    disabled={!isNameChanged}
                    className="h-8"
                  >
                    Save
                  </Button>
                </div>
                {renameError && (
                  <p className="text-xs text-destructive">{renameError}</p>
                )}
              </>
            )}

            <span className="text-sm font-medium">Project Path</span>

            <Input
              value={projectPath}
              readOnly
              disabled
              className="h-8 w-full text-xs"
            />

            <div className="flex gap-2">
              <Button
                variant="tertiary"
                size="xs"
                onClick={handleCopy}
                className="h-8 flex-1"
              >
                {isCopied ? (
                  <Check className="size-4 text-green-500" />
                ) : (
                  <Copy className="size-4" />
                )}
                <span className="ml-1.5">Copy Path</span>
              </Button>
              <Button
                variant="tertiary"
                size="xs"
                onClick={handleOpenInFinder}
                className="h-8 flex-1"
              >
                <FolderOpen className="size-4" />
                <span className="ml-1.5">Show Original</span>
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
