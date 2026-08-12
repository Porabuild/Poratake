import { describe, it, expect } from 'vitest';
import {
  formatExportTime,
  smoothRemainingSeconds,
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
