export async function loadFileAsBlob(filePath: string): Promise<Blob> {
  const response = await fetch(`file://${filePath}`);
  return response.blob();
}

export function loadImage(src: string): Promise<HTMLImageElement | null> {
  return new Promise(resolve => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

export async function writeBuffer(
  path: string,
  buffer: Uint8Array
): Promise<void> {
  await window.ipcRenderer.invoke('file:write-buffer', { path, buffer });
}
