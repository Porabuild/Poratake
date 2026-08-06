import * as settings from '@/main/settings';
import { openScreenshotEditorWithLayers } from './open-editor';

const IMAGE_BATCH_WINDOW_MS = 100;

let pendingImageFiles: string[] = [];
let imageBatchTimer: NodeJS.Timeout | null = null;

export const flushPendingImages = (): void => {
  if (imageBatchTimer) {
    clearTimeout(imageBatchTimer);
    imageBatchTimer = null;
  }

  if (pendingImageFiles.length === 0) {
    return;
  }

  const [primary, ...extras] = pendingImageFiles;
  pendingImageFiles = [];

  const edge = settings.getConfig().screenshot.multiImageAttachEdge;
  openScreenshotEditorWithLayers(primary, extras, edge);
};

export const queueImageFile = (filePath: string): void => {
  pendingImageFiles.push(filePath);

  if (imageBatchTimer) {
    clearTimeout(imageBatchTimer);
  }

  imageBatchTimer = setTimeout(flushPendingImages, IMAGE_BATCH_WINDOW_MS);
};

export const bufferImageFile = (filePath: string): void => {
  pendingImageFiles.push(filePath);
};
