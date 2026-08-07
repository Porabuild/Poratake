import { clipboard, nativeImage, screen } from 'electron';
import fs from 'fs';
import { daemon } from '@/main/daemon';
import { getConfig, updateConfig } from '@/main/settings';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
  checkAccessibilityPermission,
} from '@/main/capture/desktop-icons';
import { addToHistory } from '@/main/history';
import { generateScreenshotPath } from '@/main/capture/screenshot/utils';
import { showCapturePreview } from '@/main/capture/capture-preview';
import { openScreenshotEditor } from '@/main/capture/screenshot/open-editor';
import {
  startAreaSelection,
  cancelAreaSelection,
} from '@/main/capture/area-selector';
import type { AreaSelection } from '@/types/area';
import type { AutoScrollSpeed } from '@/types/settings';
import { isFeatureSupported } from '@/main/system/capabilities';
import { isWindows } from '@/main/utils/platform';

interface ScrollCaptureState {
  isCapturing: boolean;
  frameCount: number;
  estimatedHeight: number;
}

let activeCapture: Promise<void> | null = null;
let cancelActiveCapture: (() => Promise<void>) | null = null;

export async function startScrollCapture(): Promise<void> {
  if (!isFeatureSupported('scroll-capture')) {
    return;
  }

  if (activeCapture) {
    return activeCapture;
  }

  const capture = runScrollCapture();
  activeCapture = capture;

  try {
    await capture;
  } finally {
    if (activeCapture === capture) {
      activeCapture = null;
    }
  }
}

async function runScrollCapture(): Promise<void> {
  const config = getConfig();
  let shouldHideIcons =
    config.screenshot.hideDesktopIcons && isDesktopIconsSupported();

  if (shouldHideIcons && !checkAccessibilityPermission(false)) {
    shouldHideIcons = false;
    updateConfig({
      screenshot: { ...config.screenshot, hideDesktopIcons: false },
    });
  }

  if (shouldHideIcons) {
    await hideDesktopIcons('capture');
  }

  return new Promise<void>(resolve => {
    let areaSelected = false;
    let captureCompleted = false;
    let eventHandler: ((event: string, data: unknown) => void) | null = null;

    const cleanup = async () => {
      if (eventHandler) {
        daemon.offEvent(eventHandler);
        eventHandler = null;
      }
      if (shouldHideIcons) {
        await showDesktopIcons('capture');
      }
      cancelActiveCapture = null;
    };

    const finishCapture = async (outputPath: string) => {
      if (captureCompleted) return;
      captureCompleted = true;

      await cleanup();
      await handleCaptureComplete(outputPath);
      resolve();
    };

    const cancelCapture = async () => {
      if (captureCompleted) return;
      captureCompleted = true;

      await cleanup();
      resolve();
    };

    cancelActiveCapture = cancelCapture;

    const handleAreaSelected = async (selection: AreaSelection) => {
      if (areaSelected) return;
      if (
        selection.x === undefined ||
        selection.y === undefined ||
        selection.width === undefined ||
        selection.height === undefined
      ) {
        return;
      }

      areaSelected = true;
      await cancelAreaSelection();

      const outputPath = generateScreenshotPath();

      const scrollConfig = config.scrollCapture ?? {
        autoScrollSpeed: 'medium' as AutoScrollSpeed,
        maxHeight: 20000,
      };
      const selectionBounds = {
        x: selection.x,
        y: selection.y,
        width: selection.width,
        height: selection.height,
      };
      const captureBounds = isWindows
        ? screen.dipToScreenRect(null, selectionBounds)
        : selectionBounds;
      const scaleFactor = isWindows
        ? screen.getDisplayMatching(selectionBounds).scaleFactor
        : undefined;

      eventHandler = async (event: string) => {
        if (!event.startsWith('scroll-capture:')) return;

        const eventType = event.replace('scroll-capture:', '');

        if (eventType === 'done') {
          try {
            const result = await daemon.call<{
              success: boolean;
              outputPath: string;
              width: number;
              height: number;
            }>('scroll-capture', 'finish', { outputPath });

            if (result.success) {
              await finishCapture(result.outputPath);
            } else {
              await cancelCapture();
            }
          } catch (error) {
            console.error('Scroll capture finish failed:', error);
            await cancelCapture();
          }
        } else if (eventType === 'cancelled') {
          await cancelCapture();
        }
      };

      daemon.onEvent(eventHandler);

      try {
        await daemon.call('scroll-capture', 'start', {
          x: captureBounds.x,
          y: captureBounds.y,
          width: captureBounds.width,
          height: captureBounds.height,
          displayId: selection.screenId,
          autoScrollSpeed: scrollConfig.autoScrollSpeed,
          maxHeight: scrollConfig.maxHeight,
          scaleFactor,
        });
      } catch (error) {
        console.error('Failed to start scroll capture:', error);
        await cancelCapture();
      }
    };

    void Promise.resolve(
      startAreaSelection({
        onSelected: handleAreaSelected,
        onCancelled: async () => {
          if (areaSelected) {
            return;
          }
          await cancelCapture();
        },
        showPrompt: true,
        style: 'default',
      })
    )
      .then(async selection => {
        if (!areaSelected && selection === null) {
          await cancelCapture();
        }
      })
      .catch(async error => {
        console.error('Failed to start area selection:', error);
        await cancelCapture();
      });
  });
}

async function handleCaptureComplete(outputPath: string): Promise<void> {
  if (!fs.existsSync(outputPath)) {
    console.error('Scroll capture output file not found:', outputPath);
    return;
  }

  const config = getConfig();
  const historyItem = await addToHistory(outputPath);

  if (config.screenshot.captureToClipboard) {
    const imageBuffer = fs.readFileSync(outputPath);
    const image = nativeImage.createFromBuffer(imageBuffer);
    clipboard.writeImage(image);
    return;
  }

  if (config.screenshot.showPreview) {
    showCapturePreview(outputPath, 'screenshot', historyItem?.id);
    return;
  }

  openScreenshotEditor(outputPath, historyItem?.id);
}

export async function cancelScrollCapture(): Promise<void> {
  try {
    await daemon.call('scroll-capture', 'cancel');
  } catch (error) {
    console.error('Failed to cancel scroll capture:', error);
  }
  await cancelActiveCapture?.();
}

export async function getScrollCaptureStatus(): Promise<ScrollCaptureState> {
  try {
    const result = await daemon.call<ScrollCaptureState>(
      'scroll-capture',
      'status'
    );
    return result;
  } catch (error) {
    console.error('Failed to get scroll capture status:', error);
    return {
      isCapturing: false,
      frameCount: 0,
      estimatedHeight: 0,
    };
  }
}

export default startScrollCapture;
