import { screen } from 'electron';
import { randomUUID } from 'crypto';
import fs from 'fs';
import { daemon } from '@/main/daemon';
import { getConfig } from '@/main/settings';
import {
  hideDesktopIcons,
  showDesktopIcons,
} from '@/main/capture/desktop-icons';
import { shouldHideDesktopIconsForCapture } from '@/main/capture/desktop-icons/preference';
import {
  generateScreenshotPath,
  publishScreenshot,
} from '@/main/capture/screenshot/utils';
import {
  finalizeCapture,
  prepareScreenshotPreview,
} from '@/main/capture/screenshot/finalize';
import {
  startAreaSelection,
  confirmAreaSelection,
} from '@/main/capture/area-selector';
import {
  cancelOverlaySelection,
  selectAreaWithOverlay,
} from '@/main/capture/area-overlay';
import {
  prewarmScrollCaptureControl,
  showScrollCaptureOverlay,
  updateScrollCaptureState,
  hideScrollCaptureOverlay,
} from './scroll-capture-window';
import type {
  ScrollCaptureAction,
  ScrollCaptureOverlayState,
} from '@/types/scroll-capture';
import type { AreaSelection } from '@/types/area';
import type { AutoScrollSpeed } from '@/types/settings';
import { isFeatureSupported } from '@/main/system/capabilities';
import { isWindows } from '@/main/utils/platform';
import { showNotification } from '@/main/utils/notification';

let activeCapture: Promise<void> | null = null;
let cancelActiveCapture: (() => Promise<void>) | null = null;

export async function startScrollCapture(): Promise<void> {
  if (!isFeatureSupported('scroll-capture')) {
    return;
  }

  if (activeCapture) {
    return activeCapture;
  }

  if (!isWindows) {
    prewarmScrollCaptureControl();
  }

  const capture = runScrollCapture();
  activeCapture = capture;

  try {
    await capture;
  } finally {
    if (activeCapture === capture) {
      activeCapture = null;
      cancelActiveCapture = null;
    }
  }
}

async function runScrollCapture(): Promise<void> {
  const config = getConfig();
  const shouldHideIcons = shouldHideDesktopIconsForCapture();

  if (shouldHideIcons) {
    await hideDesktopIcons('capture');
  }

  return new Promise<void>(resolve => {
    let resolved = false;
    let cancelled = false;
    let discardFinishOutput = false;
    let overlayActive = false;
    let areaSelected = false;
    let autoScrolling = false;
    let frameCount = 0;
    let estimatedHeight = 0;
    let cursorOutside = false;
    let preview: string | null = null;
    let previewWidth: number | null = null;
    let previewHeight: number | null = null;
    let eventHandler: ((event: string, data: unknown) => void) | null = null;
    let finishInFlight: Promise<void> | null = null;
    let cancellationInFlight: Promise<void> | null = null;
    let finishPreparation: ReturnType<typeof prepareScreenshotPreview> = null;
    let outputPromoted = false;
    let publishing = false;

    const disposeFinishPreparation = () => {
      finishPreparation?.dispose();
      finishPreparation = null;
    };

    const cleanup = async () => {
      if (eventHandler) {
        daemon.offEvent(eventHandler);
        eventHandler = null;
      }
      if (overlayActive) {
        hideScrollCaptureOverlay();
        overlayActive = false;
      }
      if (shouldHideIcons) {
        await showDesktopIcons('capture');
      }
    };

    const finishCapture = async (outputPath: string, temporaryPath: string) => {
      if (resolved) return;
      resolved = true;

      try {
        await cleanup();
        if (cancelled) {
          disposeFinishPreparation();
          return;
        }

        publishing = true;
        let publishedPath: string;
        try {
          publishedPath = await publishScreenshot(temporaryPath, outputPath);
        } finally {
          publishing = false;
        }
        outputPromoted = true;
        const preparation = finishPreparation;
        finishPreparation = null;
        await handleCaptureComplete(publishedPath, preparation);
      } finally {
        resolve();
      }
    };

    const cancelCapture = (
      requestDaemon = false,
      discardOutput = requestDaemon
    ): Promise<void> => {
      if (publishing) return finishInFlight ?? Promise.resolve();
      cancelled = true;
      discardFinishOutput ||= discardOutput;
      disposeFinishPreparation();
      if (cancellationInFlight) return cancellationInFlight;
      if (resolved) return finishInFlight ?? Promise.resolve();
      resolved = true;

      const pending = requestDaemon
        ? (finishInFlight ?? daemon.call('scroll-capture', 'cancel'))
        : Promise.resolve();
      cancellationInFlight = (async () => {
        try {
          const results = await Promise.allSettled([cleanup(), pending]);
          const failure = results.find(result => result.status === 'rejected');
          if (failure?.status === 'rejected') {
            console.error(
              'Scroll capture cancellation failed:',
              failure.reason
            );
          }
        } finally {
          cancellationInFlight = null;
          resolve();
        }
      })();
      return cancellationInFlight;
    };

    const requestCaptureFinish = (): Promise<void> => {
      if (finishInFlight) return finishInFlight;
      if (resolved) return Promise.resolve();

      finishInFlight = (async () => {
        const outputPath = generateScreenshotPath();
        const temporaryPath = `${outputPath}.${randomUUID()}.tmp.png`;
        try {
          const finish = daemon.call<{ success: boolean }>(
            'scroll-capture',
            'finish',
            { outputPath: temporaryPath },
            null
          );
          finishPreparation = prepareScreenshotPreview();

          const result = await finish;
          if (result?.success) {
            await finishCapture(outputPath, temporaryPath);
            return;
          }

          if (!resolved) await cancelCapture();
        } finally {
          try {
            if (!outputPromoted && fs.existsSync(temporaryPath)) {
              if (!discardFinishOutput) {
                console.error('Scroll capture retained at:', temporaryPath);
                showNotification(
                  'Scroll Capture Not Saved',
                  `Capture data may be available at ${temporaryPath}`
                );
              } else {
                fs.unlinkSync(temporaryPath);
              }
            }
          } catch (error) {
            console.error('Failed to handle temporary scroll capture:', error);
          }
        }
      })().finally(() => {
        disposeFinishPreparation();
        finishInFlight = null;
      });

      return finishInFlight;
    };

    const pushState = () => {
      if (!overlayActive) return;

      const state: ScrollCaptureOverlayState = {
        isAutoScrolling: autoScrolling,
        cursorOutside,
        frameCount,
        estimatedHeight,
        preview,
        previewWidth,
        previewHeight,
      };
      updateScrollCaptureState(state);
    };

    const handleAction = async (action: ScrollCaptureAction) => {
      try {
        if (action === 'toggle-auto-scroll') {
          if (finishInFlight || resolved) return;
          await daemon.call(
            'scroll-capture',
            autoScrolling ? 'stopAutoScroll' : 'startAutoScroll'
          );
        } else if (action === 'done') {
          await requestCaptureFinish();
        } else if (action === 'cancel') {
          await cancelCapture(true);
        }
      } catch (error) {
        console.error('Scroll capture action failed:', error);
        await cancelCapture();
      }
    };

    const startDaemonScroll = async (params: {
      x: number;
      y: number;
      width: number;
      height: number;
      displayId?: number;
      scaleFactor?: number;
    }) => {
      if (resolved) return;

      const scrollConfig = config.scrollCapture ?? {
        autoScrollSpeed: 'medium' as AutoScrollSpeed,
        maxHeight: 20000,
      };

      try {
        await daemon.call('scroll-capture', 'start', {
          ...params,
          autoScrollSpeed: scrollConfig.autoScrollSpeed,
          maxHeight: scrollConfig.maxHeight,
        });
      } catch (error) {
        console.error('Failed to start scroll capture:', error);
        await cancelCapture();
      }
    };

    cancelActiveCapture = () => cancelCapture(true);

    eventHandler = async (event: string, data: unknown) => {
      if (!event.startsWith('scroll-capture:')) return;

      const eventType = event.replace('scroll-capture:', '');

      if (eventType === 'done') {
        try {
          await requestCaptureFinish();
        } catch (error) {
          console.error('Scroll capture finish failed:', error);
          await cancelCapture();
        }
        return;
      }

      if (eventType === 'cancelled') {
        await cancelCapture(false, true);
        return;
      }

      if (eventType === 'scroll-ended' && !resolved && !finishInFlight) {
        autoScrolling = false;
        pushState();
        const reason = (data as { reason?: unknown } | null)?.reason;
        let message: string | undefined;
        switch (reason) {
          case 'no-overlap':
            message =
              'The latest frame could not be aligned. You can save the captured portion.';
            break;
          case 'capture-error':
            message =
              'A frame could not be captured. You can save the captured portion.';
            break;
          case 'input-error':
            message =
              'Scrolling could not be continued. You can save the captured portion.';
            break;
        }
        if (message) showNotification('Scroll Capture Stopped', message);
        return;
      }

      if (!overlayActive) return;

      const payload = (data ?? {}) as Record<string, unknown>;

      if (eventType === 'frame') {
        if (payload.frameCount != null) {
          frameCount = Number(payload.frameCount);
        }
        if (payload.estimatedHeight != null) {
          estimatedHeight = Number(payload.estimatedHeight);
        }
        preview = (payload.preview as string) ?? preview;
        previewWidth = (payload.previewWidth as number) ?? previewWidth;
        previewHeight = (payload.previewHeight as number) ?? previewHeight;
        pushState();
      } else if (eventType === 'auto-scroll') {
        autoScrolling = Boolean((payload as { scrolling?: boolean }).scrolling);
        pushState();
      } else if (eventType === 'cursor') {
        cursorOutside = Boolean((payload as { outside?: boolean }).outside);
        pushState();
      }
    };

    daemon.onEvent(eventHandler);

    if (isWindows) {
      void selectAreaWithOverlay({ freeze: false })
        .then(async selection => {
          if (!selection) {
            await cancelCapture();
            return;
          }

          const bounds = screen.dipToScreenRect(null, selection.rect);
          await startDaemonScroll({
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
            scaleFactor: selection.display.scaleFactor,
          });
        })
        .catch(async error => {
          console.error('Failed to start area selection:', error);
          await cancelCapture();
        });
      return;
    }

    const handleAreaSelected = async (selection: AreaSelection) => {
      if (
        selection.x === undefined ||
        selection.y === undefined ||
        selection.width === undefined ||
        selection.height === undefined
      ) {
        return;
      }

      areaSelected = true;
      const confirmed = await confirmAreaSelection({
        keepOverlayVisible: true,
      });
      if (!confirmed || resolved) {
        await cancelCapture();
        return;
      }

      overlayActive = true;
      const overlayShown = showScrollCaptureOverlay(
        {
          displayId: selection.screenId ?? screen.getPrimaryDisplay().id,
          area: {
            x: selection.x,
            y: selection.y,
            width: selection.width,
            height: selection.height,
            displayId: selection.screenId ?? screen.getPrimaryDisplay().id,
          },
        },
        handleAction
      );
      if (!overlayShown) {
        await cancelCapture();
        return;
      }
      pushState();

      await startDaemonScroll({
        x: selection.x,
        y: selection.y,
        width: selection.width,
        height: selection.height,
        displayId: selection.screenId,
      });
    };

    void Promise.resolve(
      startAreaSelection({
        renderer: 'scroll-capture-overlay',
        onSelected: handleAreaSelected,
        onCancelled: async () => {
          if (areaSelected || resolved) {
            return;
          }
          await cancelCapture();
        },
        showPrompt: true,
      })
    )
      .then(async selection => {
        if (!areaSelected && !resolved && selection === null) {
          await cancelCapture();
        }
      })
      .catch(async error => {
        console.error('Failed to start area selection:', error);
        await cancelCapture();
      });
  });
}

async function handleCaptureComplete(
  outputPath: string,
  preparation: ReturnType<typeof prepareScreenshotPreview>
): Promise<void> {
  try {
    if (!fs.existsSync(outputPath)) {
      console.error('Scroll capture output file not found:', outputPath);
      return;
    }

    await finalizeCapture(outputPath, preparation, { silent: true });
  } finally {
    preparation?.dispose();
  }
}

export async function cancelScrollCapture(): Promise<void> {
  if (isWindows) {
    cancelOverlaySelection(true);
  }

  try {
    if (cancelActiveCapture) {
      await cancelActiveCapture();
      return;
    }
    await daemon.call('scroll-capture', 'cancel');
  } catch (error) {
    console.error('Failed to cancel scroll capture:', error);
  }
}

export default startScrollCapture;
