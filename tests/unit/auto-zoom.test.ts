import { describe, it, expect } from 'vitest';
import { generateAutoZoomSegments } from '@/main/capture/video/auto-zoom';
import type { CursorData } from '@/types/cursor';

describe('Auto Zoom Generation', () => {
  it('should return empty array for empty cursor data', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 10,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);
    expect(segments).toEqual([]);
  });

  it('should return empty array for short recordings', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [
        { timestamp: 0, x: 0.5, y: 0.5, type: 'move' },
        { timestamp: 1, x: 0.5, y: 0.5, type: 'down', button: 'left' },
      ],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 2,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);
    expect(segments).toEqual([]);
  });

  it('should return empty array for no clicks', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [
        { timestamp: 0, x: 0.5, y: 0.5, type: 'move' },
        { timestamp: 5, x: 0.6, y: 0.6, type: 'move' },
        { timestamp: 10, x: 0.7, y: 0.7, type: 'move' },
      ],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 10,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);
    expect(segments).toEqual([]);
  });

  it('should generate one zoom segment for continuous clicks', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [
        { timestamp: 1, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 2, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 3, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 4, x: 0.5, y: 0.5, type: 'down', button: 'left' },
      ],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 10,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);
    expect(segments.length).toBe(1);
    expect(segments[0].zoomLevel).toBe(2);
  });

  it('should generate multiple segments when there are gaps with no clicks', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [
        { timestamp: 1, x: 0.2, y: 0.2, type: 'down', button: 'left' },
        { timestamp: 2, x: 0.2, y: 0.2, type: 'down', button: 'left' },
        { timestamp: 10, x: 0.8, y: 0.8, type: 'down', button: 'left' },
        { timestamp: 11, x: 0.8, y: 0.8, type: 'down', button: 'left' },
      ],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 15,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);
    expect(segments.length).toBe(2);
    expect(segments[0].endTime).toBeLessThan(segments[1].startTime);
  });

  it('should deduplicate rapid duplicate clicks', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [
        { timestamp: 1, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 1.01, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 1.02, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 2, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 2.01, x: 0.5, y: 0.5, type: 'down', button: 'left' },
      ],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 10,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);
    expect(segments.length).toBe(1);
  });

  it('should have valid segment structure', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [
        { timestamp: 2, x: 0.5, y: 0.5, type: 'down', button: 'left' },
        { timestamp: 3, x: 0.5, y: 0.5, type: 'down', button: 'left' },
      ],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 10,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);

    for (const segment of segments) {
      expect(segment.id).toMatch(/^auto-zoom-\d+-\d+$/);
      expect(typeof segment.startTime).toBe('number');
      expect(typeof segment.endTime).toBe('number');
      expect(segment.startTime).toBeLessThan(segment.endTime);
      expect(segment.zoomLevel).toBe(2);
    }
  });

  it('should enforce minimum segment duration', () => {
    const cursorData: CursorData = {
      recordingArea: { width: 1920, height: 1080 },
      events: [{ timestamp: 5, x: 0.5, y: 0.5, type: 'down', button: 'left' }],
      meta: {
        startTime: '2026-01-22T18:00:00.000Z',
        duration: 15,
        sampleRate: 32,
      },
    };

    const segments = generateAutoZoomSegments(cursorData);
    expect(segments.length).toBe(1);
    const duration = segments[0].endTime - segments[0].startTime;
    expect(duration).toBeGreaterThanOrEqual(3);
  });
});
