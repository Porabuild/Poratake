import path from 'path';

export const SUPPORTED_IMAGE_EXTENSIONS = [
  '.png',
  '.jpg',
  '.jpeg',
  '.gif',
  '.webp',
  '.bmp',
  '.tiff',
  '.tif',
] as const;

export function isSupportedImageFile(filePath: string): boolean {
  const ext = path.extname(filePath).toLowerCase();
  return (SUPPORTED_IMAGE_EXTENSIONS as readonly string[]).includes(ext);
}
