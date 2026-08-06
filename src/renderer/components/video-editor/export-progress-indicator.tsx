import { useEffect, useRef, useState, useCallback } from 'react';
import { X, Check } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { CircularProgress } from '@/renderer/components/ui/circular-progress';
import { Progress } from '@/renderer/components/ui/progress';
import { cn } from '@/renderer/lib/utils';

interface ExportProgressIndicatorProps {
  isExporting: boolean;
  progress: number;
  onCancel: () => void;
}

const MIN_PROGRESS_FOR_ETA = 5;
const ETA_SMOOTHING_FACTOR = 0.1;
const COMPLETION_DISPLAY_MS = 3000;

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds) % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

export default function ExportProgressIndicator({
  isExporting,
  progress,
  onCancel,
}: ExportProgressIndicatorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [showComplete, setShowComplete] = useState(false);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [remainingSeconds, setRemainingSeconds] = useState<number | null>(null);

  const startTimeRef = useRef<number>(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const smoothedRemainingRef = useRef<number | null>(null);
  const lastProgressRef = useRef<number>(0);
  const popoverRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const wasExportingRef = useRef(false);

  useEffect(() => {
    if (isExporting) {
      wasExportingRef.current = true;
      startTimeRef.current = Date.now();
      lastProgressRef.current = 0;
      smoothedRemainingRef.current = null;
      setElapsedSeconds(0);
      setRemainingSeconds(null);
      setShowComplete(false);

      intervalRef.current = setInterval(() => {
        if (!startTimeRef.current) return;
        const elapsed = (Date.now() - startTimeRef.current) / 1000;
        setElapsedSeconds(elapsed);
      }, 100);
    } else {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    }

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [isExporting]);

  useEffect(() => {
    if (wasExportingRef.current && !isExporting && progress >= 100) {
      setShowComplete(true);
      const timer = setTimeout(() => {
        setShowComplete(false);
        setIsOpen(false);
      }, COMPLETION_DISPLAY_MS);
      wasExportingRef.current = false;
      return () => clearTimeout(timer);
    }

    if (!isExporting) {
      wasExportingRef.current = false;
    }
  }, [isExporting, progress]);

  useEffect(() => {
    if (!isExporting || !startTimeRef.current) return;
    if (progress <= MIN_PROGRESS_FOR_ETA) return;
    if (progress === lastProgressRef.current) return;

    const now = Date.now();
    const elapsed = (now - startTimeRef.current) / 1000;

    lastProgressRef.current = progress;

    const progressFraction = progress / 100;
    const estimatedTotal = elapsed / progressFraction;
    const rawRemaining = Math.max(0, estimatedTotal - elapsed);

    if (smoothedRemainingRef.current === null) {
      smoothedRemainingRef.current = rawRemaining;
    } else {
      smoothedRemainingRef.current =
        smoothedRemainingRef.current +
        ETA_SMOOTHING_FACTOR * (rawRemaining - smoothedRemainingRef.current);
    }

    setRemainingSeconds(Math.round(smoothedRemainingRef.current));
  }, [progress, isExporting]);

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
      <button
        ref={triggerRef}
        onClick={handleToggle}
        className={cn(
          'relative flex size-7 items-center justify-center rounded-md transition-colors',
          'hover:bg-accent hover:text-accent-foreground',
          isOpen && 'bg-accent text-accent-foreground'
        )}
      >
        {showComplete ? (
          <div className="bg-primary flex size-4 items-center justify-center rounded-full">
            <Check
              className="text-primary-foreground size-2.5"
              strokeWidth={3}
            />
          </div>
        ) : (
          <CircularProgress value={progress} size={16} strokeWidth={2} />
        )}
      </button>

      {isOpen && (
        <div
          ref={popoverRef}
          className={cn(
            'bg-popover text-popover-foreground border-border absolute top-full right-0 z-50 mt-1.5 w-64 rounded-lg border shadow-lg',
            'animate-in fade-in-0 zoom-in-95 data-[side=bottom]:slide-in-from-top-2'
          )}
        >
          <div className="space-y-3 p-3">
            {showComplete ? (
              <div className="flex items-center gap-2">
                <div className="bg-primary flex size-5 items-center justify-center rounded-full">
                  <Check
                    className="text-primary-foreground size-3"
                    strokeWidth={3}
                  />
                </div>
                <span className="text-sm font-medium">Export Complete</span>
              </div>
            ) : (
              <>
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium">Exporting...</span>
                  <span className="text-muted-foreground text-xs tabular-nums">
                    {Math.round(progress)}%
                  </span>
                </div>

                <Progress value={progress} className="h-1.5" />

                <div className="flex items-center justify-between">
                  <span className="text-muted-foreground text-xs tabular-nums">
                    {formatTime(elapsedSeconds)} elapsed
                  </span>
                  <span className="text-muted-foreground text-xs tabular-nums">
                    {remainingSeconds !== null
                      ? `${formatTime(remainingSeconds)} remaining`
                      : 'Calculating...'}
                  </span>
                </div>

                <Button
                  variant="outline"
                  size="sm"
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
