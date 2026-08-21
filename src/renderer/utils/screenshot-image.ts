import { loadImage } from '@/renderer/utils/image';

export { loadImage as loadImageSource };

export async function loadScreenshotImage(
  imageUrl: string | undefined,
  readFile: () => Promise<string>
): Promise<HTMLImageElement> {
  if (imageUrl) {
    const directImage = await loadImage(imageUrl).catch(() => null);
    if (directImage) return directImage;
  }

  const base64 = await readFile();
  return loadImage(`data:image/png;base64,${base64}`);
}
