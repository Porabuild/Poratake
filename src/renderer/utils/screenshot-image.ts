export function loadImageSource(source: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error('Failed to load image'));
    image.src = source;
  });
}

export async function loadScreenshotImage(
  imageUrl: string | undefined,
  readFile: () => Promise<string>
): Promise<HTMLImageElement> {
  if (imageUrl) {
    const directImage = await loadImageSource(imageUrl).catch(() => null);
    if (directImage) return directImage;
  }

  const base64 = await readFile();
  return loadImageSource(`data:image/png;base64,${base64}`);
}
