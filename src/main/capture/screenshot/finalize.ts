import { clipboard, nativeImage } from 'electron';
import fs from 'fs';
import { getConfig } from '@/main/settings';
import { addToHistory } from '@/main/history';
import {
  prepareCapturePreview,
  showCapturePreview,
} from '@/main/capture/capture-preview';
import type {
  CapturePreviewHandle,
  CapturePreviewPreparation,
} from '@/main/capture/capture-preview';
import { openScreenshotEditor } from '@/main/capture/screenshot/open-editor';
import { playCaptureSound } from '@/main/capture/screenshot/capture-sound';

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

export function prepareScreenshotPreview(): CapturePreviewPreparation | null {
  const { screenshot } = getConfig();
  if (!screenshot.showPreview || screenshot.captureToClipboard) {
    return null;
  }

  try {
    return prepareCapturePreview();
  } catch (error) {
    console.error('Failed to prepare capture preview:', error);
    return null;
  }
}

export interface FinalizeCaptureOptions {
  silent?: boolean;
}

export async function finalizeCapture(
  filePath: string,
  preparation?: CapturePreviewPreparation | null,
  options?: FinalizeCaptureOptions
): Promise<void> {
  if (!fs.existsSync(filePath)) {
    preparation?.dispose();
    return;
  }

  if (!options?.silent) {
    playCaptureSound();
  }

  const { screenshot } = getConfig();
  let startHistoryPersistence: () => void = () => {};
  const historyStart = new Promise<void>(resolve => {
    startHistoryPersistence = resolve;
  });
  const historyItemPromise = historyStart.then(() => addToHistory(filePath));
  const historyIdPromise = historyItemPromise.then(item => item?.id);
  let preview: CapturePreviewHandle | null = null;

  if (screenshot.showPreview && !screenshot.captureToClipboard) {
    try {
      preview = showCapturePreview(
        filePath,
        'screenshot',
        undefined,
        preparation ?? undefined,
        historyIdPromise
      );
    } catch (error) {
      console.error('Failed to show capture preview:', error);
    }
  }

  if (!preview) {
    preparation?.dispose();
    startHistoryPersistence();
  } else {
    void preview.revealed.then(startHistoryPersistence);
  }

  let clipboardPromise = Promise.resolve();

  if (screenshot.autoCopyToClipboard || screenshot.captureToClipboard) {
    if (preview) {
      clipboardPromise = preview.revealed.then(() =>
        copyImageFileToClipboard(filePath)
      );
    } else {
      copyImageFileToClipboard(filePath);
    }
  }

  const [historyItem] = await Promise.all([
    historyItemPromise,
    clipboardPromise,
  ]);

  if (screenshot.captureToClipboard) {
    return;
  }

  if (preview) {
    return;
  }

  openScreenshotEditor(filePath, historyItem?.id);
}
