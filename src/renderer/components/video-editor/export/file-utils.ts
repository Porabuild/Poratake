import { toFileUrl } from '../utils';
export { loadImageOrNull as loadImage } from '@/renderer/utils/image';

export async function loadFileAsBlob(filePath: string): Promise<Blob> {
  const response = await fetch(toFileUrl(filePath));
  return response.blob();
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
