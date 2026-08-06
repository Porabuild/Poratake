import { describe, it, expect } from 'vitest';
import { getAspectRatioValue } from '@/renderer/hooks/useContentDimensions';
import type { AspectRatioOption } from '@/types/editor';

describe('Aspect Ratio', () => {
  describe('getAspectRatioValue', () => {
    it('should return null for auto', () => {
      expect(getAspectRatioValue('auto')).toBeNull();
    });

    it('should return 1 for 1:1', () => {
      expect(getAspectRatioValue('1:1')).toBe(1);
    });

    it('should return correct values for landscape ratios', () => {
      expect(getAspectRatioValue('4:3')).toBeCloseTo(4 / 3);
      expect(getAspectRatioValue('3:2')).toBeCloseTo(3 / 2);
      expect(getAspectRatioValue('16:9')).toBeCloseTo(16 / 9);
      expect(getAspectRatioValue('16:10')).toBeCloseTo(16 / 10);
      expect(getAspectRatioValue('21:9')).toBeCloseTo(21 / 9);
    });

    it('should return correct values for portrait ratios', () => {
      expect(getAspectRatioValue('9:16')).toBeCloseTo(9 / 16);
      expect(getAspectRatioValue('3:4')).toBeCloseTo(3 / 4);
      expect(getAspectRatioValue('2:3')).toBeCloseTo(2 / 3);
    });

    it('should return null for unknown values', () => {
      expect(getAspectRatioValue('unknown' as AspectRatioOption)).toBeNull();
    });
  });

  describe('aspect ratio padding calculation', () => {
    function calcPadding(
      baseWidth: number,
      baseHeight: number,
      aspectRatio: AspectRatioOption
    ) {
      const targetRatio = getAspectRatioValue(aspectRatio);
      let paddingX = 0;
      let paddingY = 0;

      if (targetRatio !== null && baseWidth > 0 && baseHeight > 0) {
        const currentRatio = baseWidth / baseHeight;

        if (currentRatio < targetRatio) {
          const newWidth = baseHeight * targetRatio;
          paddingX = (newWidth - baseWidth) / 2;
        } else if (currentRatio > targetRatio) {
          const newHeight = baseWidth / targetRatio;
          paddingY = (newHeight - baseHeight) / 2;
        }
      }

      return {
        paddingX,
        paddingY,
        canvasWidth: baseWidth + paddingX * 2,
        canvasHeight: baseHeight + paddingY * 2,
      };
    }

    it('should return zero padding for auto', () => {
      const result = calcPadding(800, 600, 'auto');
      expect(result.paddingX).toBe(0);
      expect(result.paddingY).toBe(0);
    });

    it('should add horizontal padding when content is too narrow', () => {
      const result = calcPadding(400, 400, '16:9');
      expect(result.paddingX).toBeGreaterThan(0);
      expect(result.paddingY).toBe(0);
      expect(result.canvasWidth / result.canvasHeight).toBeCloseTo(16 / 9);
    });

    it('should add vertical padding when content is too wide', () => {
      const result = calcPadding(800, 200, '1:1');
      expect(result.paddingX).toBe(0);
      expect(result.paddingY).toBeGreaterThan(0);
      expect(result.canvasWidth / result.canvasHeight).toBeCloseTo(1);
    });

    it('should add no padding when content already matches ratio', () => {
      const result = calcPadding(1600, 900, '16:9');
      expect(result.paddingX).toBeCloseTo(0);
      expect(result.paddingY).toBeCloseTo(0);
    });

    it('should handle zero-dimension content gracefully', () => {
      const zeroWidth = calcPadding(0, 600, '16:9');
      expect(zeroWidth.paddingX).toBe(0);
      expect(zeroWidth.paddingY).toBe(0);

      const zeroHeight = calcPadding(800, 0, '16:9');
      expect(zeroHeight.paddingX).toBe(0);
      expect(zeroHeight.paddingY).toBe(0);

      const zeroBoth = calcPadding(0, 0, '16:9');
      expect(zeroBoth.paddingX).toBe(0);
      expect(zeroBoth.paddingY).toBe(0);
    });

    it('should produce correct ratio for extreme aspect ratios', () => {
      const ultrawide = calcPadding(400, 800, '21:9');
      expect(ultrawide.paddingX).toBeGreaterThan(0);
      expect(ultrawide.paddingY).toBe(0);
      expect(ultrawide.canvasWidth / ultrawide.canvasHeight).toBeCloseTo(
        21 / 9
      );

      const portrait = calcPadding(1920, 1080, '9:16');
      expect(portrait.paddingX).toBe(0);
      expect(portrait.paddingY).toBeGreaterThan(0);
      expect(portrait.canvasWidth / portrait.canvasHeight).toBeCloseTo(9 / 16);
    });

    it('should produce symmetric padding', () => {
      const result = calcPadding(400, 400, '16:9');
      const expectedWidth = 400 * (16 / 9);
      const expectedPaddingX = (expectedWidth - 400) / 2;
      expect(result.paddingX).toBeCloseTo(expectedPaddingX);
      expect(result.canvasWidth).toBeCloseTo(expectedWidth);
      expect(result.canvasHeight).toBe(400);
    });
  });
});
