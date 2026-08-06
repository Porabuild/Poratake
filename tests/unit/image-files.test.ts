import { describe, it, expect } from 'vitest';
import {
  isSupportedImageFile,
  SUPPORTED_IMAGE_EXTENSIONS,
} from '@/main/utils/image-files';

describe('image-files', () => {
  describe('SUPPORTED_IMAGE_EXTENSIONS', () => {
    it('contains common image extensions', () => {
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('.png');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('.jpg');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('.jpeg');
      expect(SUPPORTED_IMAGE_EXTENSIONS).toContain('.webp');
    });
  });

  describe('isSupportedImageFile', () => {
    it('returns true for supported extensions', () => {
      expect(isSupportedImageFile('/path/to/image.png')).toBe(true);
      expect(isSupportedImageFile('/path/to/photo.jpg')).toBe(true);
      expect(isSupportedImageFile('/path/to/photo.jpeg')).toBe(true);
      expect(isSupportedImageFile('/path/to/anim.gif')).toBe(true);
      expect(isSupportedImageFile('/path/to/web.webp')).toBe(true);
      expect(isSupportedImageFile('/path/to/raster.bmp')).toBe(true);
      expect(isSupportedImageFile('/path/to/scan.tiff')).toBe(true);
      expect(isSupportedImageFile('/path/to/scan.tif')).toBe(true);
    });

    it('is case-insensitive', () => {
      expect(isSupportedImageFile('/path/to/photo.PNG')).toBe(true);
      expect(isSupportedImageFile('/path/to/photo.JPG')).toBe(true);
      expect(isSupportedImageFile('/path/to/scan.TIFF')).toBe(true);
    });

    it('returns false for unsupported extensions', () => {
      expect(isSupportedImageFile('/path/to/video.mp4')).toBe(false);
      expect(isSupportedImageFile('/path/to/doc.pdf')).toBe(false);
      expect(isSupportedImageFile('/path/to/no-extension')).toBe(false);
    });
  });
});
