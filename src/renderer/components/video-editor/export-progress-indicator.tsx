import { useEffect, useRef, useState, useCallback } from 'react';
import { X, Check } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { CircularProgress } from '@/renderer/components/ui/circular-progress';
import { Progress } from '@/renderer/components/ui/progress';
import {
  useExportProgress,
  formatExportTime,
} from './hooks/use-export-progress';
import { cn } from '@/renderer/lib/utils';

interface ExportProgressIndicatorProps {
  isExporting: boolean;
  progress: number;
  onCancel: () => void;
}

const COMPLETION_DISPLAY_MS = 3000;

export default function ExportProgressIndicator({
  isExporting,
  progress,
  onCancel,
}: ExportProgressIndicatorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [showComplete, setShowComplete] = useState(false);

  const [previousIsExporting, setPreviousIsExporting] = useState(isExporting);
  const popoverRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  const { elapsedSeconds, remainingSeconds } = useExportProgress({
    isExporting,
    progress,
  });

  if (previousIsExporting !== isExporting) {
    setPreviousIsExporting(isExporting);
    if (isExporting) {
      setShowComplete(false);
    } else if (progress >= 100) {
      setShowComplete(true);
    }
  }

  useEffect(() => {
    if (!showComplete) return;
    const timer = setTimeout(() => {
      setShowComplete(false);
      setIsOpen(false);
    }, COMPLETION_DISPLAY_MS);
    return () => clearTimeout(timer);
  }, [showComplete]);

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

  const handleToggle = useCallback(() => {
    setIsOpen(prev => !prev);
  }, []);

  const handleCancel = useCallback(() => {
    onCancel();
    setIsOpen(false);
  }, [onCancel]);

  const isVisible = isExporting || showComplete;
  if (!isVisible) return null;

  return (
    <div className="no-drag relative flex items-center">
      <Button
        ref={triggerRef}
        variant={isOpen ? 'tertiary' : 'ghost'}
        size="icon-xs"
        className="size-7!"
        onClick={handleToggle}
      >
        {showComplete ? (
          <div className="flex size-4 items-center justify-center rounded-full bg-primary">
            <Check
              className="size-2.5 text-primary-foreground"
              strokeWidth={3}
            />
          </div>
        ) : (
          <CircularProgress value={progress} size={16} strokeWidth={2} />
        )}
      </Button>

      {isOpen && (
        <div
          ref={popoverRef}
          className={cn(
            'absolute top-full right-0 z-50 mt-1.5 w-64 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg',
            'animate-in fade-in-0 zoom-in-95 data-[side=bottom]:slide-in-from-top-2'
          )}
        >
          <div className="space-y-3 p-3">
            {showComplete ? (
              <div className="flex items-center gap-2">
                <div className="flex size-5 items-center justify-center rounded-full bg-primary">
                  <Check
                    className="size-3 text-primary-foreground"
                    strokeWidth={3}
                  />
                </div>
                <span className="text-sm font-medium">Export Complete</span>
              </div>
            ) : (
              <>
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium">Exporting...</span>
                  <span className="text-xs text-muted-foreground tabular-nums">
                    {Math.round(progress)}%
                  </span>
                </div>

                <Progress value={progress} className="h-1.5" />

                <div className="flex items-center justify-between">
                  <span className="text-xs text-muted-foreground tabular-nums">
                    {formatExportTime(elapsedSeconds)} elapsed
                  </span>
                  <span className="text-xs text-muted-foreground tabular-nums">
                    {remainingSeconds !== null
                      ? `${formatExportTime(remainingSeconds)} remaining`
                      : 'Calculating...'}
                  </span>
                </div>

                <Button
                  variant="tertiary"
                  size="xs"
                  onClick={handleCancel}
                  className="w-full"
                >
                  <X className="mr-1 size-3.5" />
                  Cancel
                </Button>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
