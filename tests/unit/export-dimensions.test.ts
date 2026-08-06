import { describe, it, expect } from 'vitest';
import { calculateExportDimensions } from '@/renderer/components/video-editor/export/export-dimensions';

describe('calculateExportDimensions', () => {
  describe('resolution scaling', () => {
    it('should return original dimensions when resolution is original', () => {
      const result = calculateExportDimensions(1920, 1080, 0, 'original');

      expect(result.width).toBe(1920);
      expect(result.height).toBe(1080);
      expect(result.scale).toBe(1);
    });

    it('should scale to 1080p resolution', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '1080p');

      expect(result.height).toBe(1080);
      expect(result.width).toBe(1920);
      expect(result.scale).toBe(1);
    });

    it('should scale to 720p resolution', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '720p');

      expect(result.height).toBe(720);
      expect(result.width).toBe(1280);
      expect(result.scale).toBeCloseTo(720 / 1080, 5);
    });

    it('should scale to 480p resolution', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '480p');

      expect(result.height).toBe(480);
      expect(result.width).toBe(854);
      expect(result.scale).toBeCloseTo(480 / 1080, 5);
    });

    it('should scale to 4k resolution', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '4k');

      expect(result.height).toBe(2160);
      expect(result.width).toBe(3840);
      expect(result.scale).toBe(2);
    });
  });

  describe('padding handling', () => {
    it('should include padding in composition dimensions', () => {
      const result = calculateExportDimensions(1920, 1080, 50, 'original');

      expect(result.width).toBe(2020);
      expect(result.height).toBe(1180);
      expect(result.scale).toBe(1);
    });

    it('should maintain aspect ratio with padding when scaling', () => {
      const padding = 100;
      const videoWidth = 1920;
      const videoHeight = 1080;
      const compositionWidth = videoWidth + padding * 2;
      const compositionHeight = videoHeight + padding * 2;
      const expectedAspectRatio = compositionWidth / compositionHeight;

      const result = calculateExportDimensions(
        videoWidth,
        videoHeight,
        padding,
        '720p'
      );

      const actualAspectRatio = result.width / result.height;
      expect(actualAspectRatio).toBeCloseTo(expectedAspectRatio, 1);
    });
  });

  describe('even dimension enforcement', () => {
    it('should ensure width is even', () => {
      const result = calculateExportDimensions(1919, 1080, 0, 'original');

      expect(result.width % 2).toBe(0);
    });

    it('should ensure height is even', () => {
      const result = calculateExportDimensions(1920, 1079, 0, 'original');

      expect(result.height % 2).toBe(0);
    });

    it('should ensure both dimensions are even after scaling', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '720p');

      expect(result.width % 2).toBe(0);
      expect(result.height % 2).toBe(0);
    });
  });

  describe('H.264 limits', () => {
    it('should not exceed MAX_H264_DIMENSION for width', () => {
      const result = calculateExportDimensions(8000, 4000, 0, 'original');

      expect(result.width).toBeLessThanOrEqual(4096);
    });

    it('should not exceed MAX_H264_DIMENSION for height', () => {
      const result = calculateExportDimensions(4000, 8000, 0, 'original');

      expect(result.height).toBeLessThanOrEqual(4096);
    });

    it('should not exceed MAX_H264_PIXELS total', () => {
      const result = calculateExportDimensions(4000, 3000, 0, 'original');
      const totalPixels = result.width * result.height;

      expect(totalPixels).toBeLessThanOrEqual(8847360);
    });

    it('should maintain even dimensions after H.264 scaling', () => {
      const result = calculateExportDimensions(8000, 4000, 0, 'original');

      expect(result.width % 2).toBe(0);
      expect(result.height % 2).toBe(0);
    });

    it('should maintain aspect ratio after H.264 scaling', () => {
      const videoWidth = 6000;
      const videoHeight = 3000;
      const originalAspectRatio = videoWidth / videoHeight;

      const result = calculateExportDimensions(
        videoWidth,
        videoHeight,
        0,
        'original'
      );

      const resultAspectRatio = result.width / result.height;
      expect(resultAspectRatio).toBeCloseTo(originalAspectRatio, 1);
    });
  });

  describe('scale calculation', () => {
    it('should return scale of 1 when no scaling needed', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '1080p');

      expect(result.scale).toBe(1);
    });

    it('should return scale less than 1 when downscaling', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '720p');

      expect(result.scale).toBeLessThan(1);
    });

    it('should return scale greater than 1 when upscaling', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '4k');

      expect(result.scale).toBeGreaterThan(1);
    });

    it('should calculate scale based on composition height', () => {
      const padding = 50;
      const videoHeight = 1080;
      const compositionHeight = videoHeight + padding * 2;
      const targetHeight = 720;
      const expectedScale = targetHeight / compositionHeight;

      const result = calculateExportDimensions(
        1920,
        videoHeight,
        padding,
        '720p'
      );

      expect(result.scale).toBeCloseTo(expectedScale, 5);
    });
  });

  describe('edge cases', () => {
    it('should handle zero padding', () => {
      const result = calculateExportDimensions(1920, 1080, 0, '1080p');

      expect(result.width).toBe(1920);
      expect(result.height).toBe(1080);
    });

    it('should handle small video dimensions', () => {
      const result = calculateExportDimensions(320, 240, 0, '480p');

      expect(result.width).toBeGreaterThan(0);
      expect(result.height).toBeGreaterThan(0);
      expect(result.width % 2).toBe(0);
      expect(result.height % 2).toBe(0);
    });

    it('should handle non-standard aspect ratios', () => {
      const result = calculateExportDimensions(1000, 1000, 0, '720p');

      expect(result.height).toBe(720);
      expect(result.width).toBe(720);
      expect(result.width % 2).toBe(0);
      expect(result.height % 2).toBe(0);
    });

    it('should handle ultrawide aspect ratios', () => {
      const result = calculateExportDimensions(3440, 1440, 0, '1080p');

      expect(result.height).toBe(1080);
      expect(result.width % 2).toBe(0);
      const aspectRatio = result.width / result.height;
      expect(aspectRatio).toBeCloseTo(3440 / 1440, 1);
    });
  });
});
