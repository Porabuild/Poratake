export function loadImage(source: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`Failed to load image: ${source}`));
    image.src = source;
  });
}

export function loadImageOrNull(
  source: string
): Promise<HTMLImageElement | null> {
  return loadImage(source).catch(() => null);
}

export function loadImageFromBase64(base64: string): Promise<HTMLImageElement> {
  return loadImage(`data:image/png;base64,${base64}`);
}
