import { toFileUrl } from '../utils';

export async function loadFileAsBlob(filePath: string): Promise<Blob> {
  const response = await fetch(toFileUrl(filePath));
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

export async function createOutputFile(path: string): Promise<void> {
  const result = (await window.ipcRenderer.invoke('file:create-output', {
    path,
  })) as { success: boolean; error?: string };

  if (!result.success) {
    throw new Error(result.error ?? 'Failed to create output file');
  }
}

export async function writeOutputChunk(
  path: string,
  position: number,
  buffer: Uint8Array
): Promise<void> {
  const result = (await window.ipcRenderer.invoke('file:write-output-chunk', {
    path,
    position,
    buffer,
  })) as { success: boolean; error?: string };

  if (!result.success) {
    throw new Error(result.error ?? 'Failed to write file');
  }
}
