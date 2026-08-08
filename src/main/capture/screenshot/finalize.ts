import { clipboard, nativeImage } from 'electron';
import fs from 'fs';
import { getConfig } from '@/main/settings';
import { addToHistory } from '@/main/history';
import { showCapturePreview } from '@/main/capture/capture-preview';
import { openScreenshotEditor } from '@/main/capture/screenshot/open-editor';

export function copyImageFileToClipboard(filePath: string): void {
  try {
    const image = nativeImage.createFromBuffer(fs.readFileSync(filePath));
    if (image.isEmpty()) {
      console.error('Failed to copy screenshot to clipboard: empty image');
      return;
    }
    clipboard.writeImage(image);
  } catch (error) {
    console.error('Failed to copy screenshot to clipboard:', error);
  }
}

export async function finalizeCapture(filePath: string): Promise<void> {
  if (!fs.existsSync(filePath)) {
    return;
  }

  const { screenshot } = getConfig();
  const historyItem = await addToHistory(filePath);

  if (screenshot.autoCopyToClipboard || screenshot.captureToClipboard) {
    copyImageFileToClipboard(filePath);
  }

  if (screenshot.captureToClipboard) {
    return;
  }

  if (screenshot.showPreview) {
    showCapturePreview(filePath, 'screenshot', historyItem?.id);
    return;
  }

  openScreenshotEditor(filePath, historyItem?.id);
}
