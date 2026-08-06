import { describe, it, expect } from 'vitest';

import { calculateBitrate } from '../../src/renderer/components/video-editor/export/bitrate';

describe('video export bitrate', () => {
  describe('quality presets', () => {
    it('uses a higher bitrate for social preset at 1080p60', () => {
      const bitrate = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 60,
        qualityPreset: 'social',
        hasCamera: false,
      });

      expect(bitrate).toBeGreaterThanOrEqual(8_000_000);
    });

    it('studio preset has highest bitrate', () => {
      const studioBitrate = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'studio',
      });

      const socialBitrate = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
      });

      expect(studioBitrate).toBeGreaterThan(socialBitrate);
    });

    it('web-low preset has lowest bitrate', () => {
      const webLowBitrate = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'web-low',
      });

      const webBitrate = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'web',
      });

      expect(webLowBitrate).toBeLessThan(webBitrate);
    });

    it('preset order is studio > social > web > web-low', () => {
      const params = { width: 1920, height: 1080, fps: 30 };

      const studio = calculateBitrate({ ...params, qualityPreset: 'studio' });
      const social = calculateBitrate({ ...params, qualityPreset: 'social' });
      const web = calculateBitrate({ ...params, qualityPreset: 'web' });
      const webLow = calculateBitrate({ ...params, qualityPreset: 'web-low' });

      expect(studio).toBeGreaterThan(social);
      expect(social).toBeGreaterThan(web);
      expect(web).toBeGreaterThan(webLow);
    });
  });

  describe('resolution scaling', () => {
    it('higher resolution requires higher bitrate', () => {
      const bitrate1080p = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
      });

      const bitrate4k = calculateBitrate({
        width: 3840,
        height: 2160,
        fps: 30,
        qualityPreset: 'social',
      });

      expect(bitrate4k).toBeGreaterThan(bitrate1080p);
    });

    it('720p has lower or equal bitrate than 1080p (may be clamped by min)', () => {
      const bitrate720p = calculateBitrate({
        width: 1280,
        height: 720,
        fps: 30,
        qualityPreset: 'social',
      });

      const bitrate1080p = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
      });

      expect(bitrate720p).toBeLessThanOrEqual(bitrate1080p);
    });
  });

  describe('frame rate scaling', () => {
    it('higher fps requires higher bitrate', () => {
      const bitrate30fps = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
      });

      const bitrate60fps = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 60,
        qualityPreset: 'social',
      });

      expect(bitrate60fps).toBeGreaterThan(bitrate30fps);
    });

    it('24fps has lower or equal bitrate than 30fps (may be clamped by min)', () => {
      const bitrate24fps = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 24,
        qualityPreset: 'studio',
      });

      const bitrate30fps = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'studio',
      });

      expect(bitrate24fps).toBeLessThanOrEqual(bitrate30fps);
    });
  });

  describe('camera content factor', () => {
    it('screen with camera has higher or equal bitrate than screen only', () => {
      const screenOnly = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
        hasCamera: false,
      });

      const withCamera = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
        hasCamera: true,
      });

      expect(withCamera).toBeGreaterThanOrEqual(screenOnly);
    });

    it('defaults to no camera when hasCamera is undefined', () => {
      const withoutParam = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
      });

      const explicitFalse = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
        hasCamera: false,
      });

      expect(withoutParam).toBe(explicitFalse);
    });
  });

  describe('bitrate clamping', () => {
    it('studio preset respects minimum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 320,
        height: 240,
        fps: 10,
        qualityPreset: 'studio',
      });

      expect(bitrate).toBeGreaterThanOrEqual(12_000_000);
    });

    it('studio preset respects maximum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 7680,
        height: 4320,
        fps: 60,
        qualityPreset: 'studio',
      });

      expect(bitrate).toBeLessThanOrEqual(100_000_000);
    });

    it('social preset respects minimum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 320,
        height: 240,
        fps: 10,
        qualityPreset: 'social',
      });

      expect(bitrate).toBeGreaterThanOrEqual(8_000_000);
    });

    it('social preset respects maximum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 3840,
        height: 2160,
        fps: 60,
        qualityPreset: 'social',
      });

      expect(bitrate).toBeLessThanOrEqual(16_000_000);
    });

    it('web preset respects minimum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 320,
        height: 240,
        fps: 10,
        qualityPreset: 'web',
      });

      expect(bitrate).toBeGreaterThanOrEqual(1_500_000);
    });

    it('web preset respects maximum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 3840,
        height: 2160,
        fps: 60,
        qualityPreset: 'web',
      });

      expect(bitrate).toBeLessThanOrEqual(4_000_000);
    });

    it('web-low preset respects minimum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 320,
        height: 240,
        fps: 10,
        qualityPreset: 'web-low',
      });

      expect(bitrate).toBeGreaterThanOrEqual(600_000);
    });

    it('web-low preset respects maximum bitrate', () => {
      const bitrate = calculateBitrate({
        width: 3840,
        height: 2160,
        fps: 60,
        qualityPreset: 'web-low',
      });

      expect(bitrate).toBeLessThanOrEqual(1_500_000);
    });
  });

  describe('output format', () => {
    it('returns bitrate as integer', () => {
      const bitrate = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
      });

      expect(Number.isInteger(bitrate)).toBe(true);
    });

    it('returns bitrate in bits per second (not megabits)', () => {
      const bitrate = calculateBitrate({
        width: 1920,
        height: 1080,
        fps: 30,
        qualityPreset: 'social',
      });

      expect(bitrate).toBeGreaterThan(1_000_000);
    });
  });
});
