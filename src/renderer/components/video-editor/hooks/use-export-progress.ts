import { useEffect, useRef, useState } from 'react';

const MIN_PROGRESS_FOR_ETA = 5;
export const ETA_SMOOTHING_FACTOR = 0.1;
const ELAPSED_TICK_MS = 100;

export function formatExportTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds) % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

export function smoothRemainingSeconds(
  previousRemaining: number | null,
  elapsedSeconds: number,
  progress: number
): number {
  const estimatedTotal = elapsedSeconds / (progress / 100);
  const rawRemaining = Math.max(0, estimatedTotal - elapsedSeconds);

  if (previousRemaining === null) {
    return rawRemaining;
  }

  return (
    previousRemaining +
    ETA_SMOOTHING_FACTOR * (rawRemaining - previousRemaining)
  );
}

interface UseExportProgressOptions {
  isExporting: boolean;
  progress: number;
}

interface ExportProgressState {
  elapsedSeconds: number;
  remainingSeconds: number | null;
}

export function useExportProgress({
  isExporting,
  progress,
}: UseExportProgressOptions): ExportProgressState {
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [remainingSeconds, setRemainingSeconds] = useState<number | null>(null);

  const startTimeRef = useRef<number>(0);
  const smoothedRemainingRef = useRef<number | null>(null);
  const progressRef = useRef<number>(progress);

  useEffect(() => {
    progressRef.current = progress;
  }, [progress]);

  useEffect(() => {
    if (!isExporting) return;

    startTimeRef.current = Date.now();
    smoothedRemainingRef.current = null;
    setElapsedSeconds(0);
    setRemainingSeconds(null);

    const interval = setInterval(() => {
      if (!startTimeRef.current) return;

      const elapsed = (Date.now() - startTimeRef.current) / 1000;
      setElapsedSeconds(elapsed);

      const currentProgress = progressRef.current;
      if (currentProgress <= MIN_PROGRESS_FOR_ETA) return;

      smoothedRemainingRef.current = smoothRemainingSeconds(
        smoothedRemainingRef.current,
        elapsed,
        currentProgress
      );
      setRemainingSeconds(Math.round(smoothedRemainingRef.current));
    }, ELAPSED_TICK_MS);

    return () => clearInterval(interval);
  }, [isExporting]);

  return { elapsedSeconds, remainingSeconds };
}
