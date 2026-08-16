// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest';
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import {
  formatExportTime,
  smoothRemainingSeconds,
  useExportProgress,
  ETA_SMOOTHING_FACTOR,
} from '@/renderer/components/video-editor/hooks/use-export-progress';

describe('formatExportTime', () => {
  it('formats sub-minute durations with padded seconds', () => {
    expect(formatExportTime(0)).toBe('0:00');
    expect(formatExportTime(5)).toBe('0:05');
    expect(formatExportTime(59.9)).toBe('0:59');
  });

  it('formats minute-based durations', () => {
    expect(formatExportTime(60)).toBe('1:00');
    expect(formatExportTime(65)).toBe('1:05');
    expect(formatExportTime(3661)).toBe('61:01');
  });
});

describe('smoothRemainingSeconds', () => {
  it('returns the raw remaining time on the first estimate', () => {
    expect(smoothRemainingSeconds(null, 10, 50)).toBe(10);
    expect(smoothRemainingSeconds(null, 1, 6)).toBeCloseTo(15.67, 1);
  });

  it('moves towards the raw estimate by the smoothing factor', () => {
    const result = smoothRemainingSeconds(20, 10, 50);
    expect(result).toBeCloseTo(20 + ETA_SMOOTHING_FACTOR * (10 - 20), 5);
  });

  it('stays unchanged when the raw estimate matches the previous one', () => {
    expect(smoothRemainingSeconds(10, 10, 50)).toBeCloseTo(10, 5);
  });

  it('never returns a negative remaining time', () => {
    expect(smoothRemainingSeconds(null, 10, 100)).toBe(0);
    expect(smoothRemainingSeconds(null, 10, 120)).toBe(0);
  });
});

describe('useExportProgress', () => {
  let container: HTMLDivElement;
  let root: Root;
  let latest: { elapsedSeconds: number; remainingSeconds: number | null };

  function Probe({
    isExporting,
    progress,
  }: {
    isExporting: boolean;
    progress: number;
  }): null {
    latest = useExportProgress({ isExporting, progress });
    return null;
  }

  function render(isExporting: boolean, progress: number): void {
    act(() => {
      root.render(createElement(Probe, { isExporting, progress }));
    });
  }

  beforeEach(() => {
    vi.useFakeTimers();
    (
      globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
    ).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('keeps updating the remaining time while progress stays constant', () => {
    render(true, 10);

    act(() => {
      vi.advanceTimersByTime(100);
    });
    const firstRemaining = latest.remainingSeconds;
    expect(firstRemaining).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(10000);
    });

    expect(latest.elapsedSeconds).toBeGreaterThan(10);
    expect(latest.remainingSeconds).toBeGreaterThan(firstRemaining as number);
  });

  it('holds the remaining time empty below the minimum progress', () => {
    render(true, 2);

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(latest.remainingSeconds).toBeNull();
    expect(latest.elapsedSeconds).toBeGreaterThan(0);
  });

  it('resets elapsed and remaining time when a new export starts', () => {
    render(true, 50);
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    expect(latest.remainingSeconds).not.toBeNull();

    render(false, 0);
    render(true, 0);

    expect(latest.elapsedSeconds).toBe(0);
    expect(latest.remainingSeconds).toBeNull();
  });
});
