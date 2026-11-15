import fs from 'fs';
import { getConfig } from '@/main/settings';
import { captureScreenshotToUniqueFile } from './utils.ts';
import type { AreaSelection } from '@/types/area.ts';
import {
  finalizeCapture,
  prepareScreenshotPreview,
} from '@/main/capture/screenshot/finalize';
import {
  captureFrozenWindowToFile,
  captureRegionToFile,
  captureWindowByIdToFile,
} from '@/main/capture/screenshot/native-capture';
import type { CaptureFailureHandler } from '@/main/capture/screenshot/native-capture';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
  checkAccessibilityPermission,
} from '@/main/capture/desktop-icons';

export interface CaptureAreaOptions {
  cached?: boolean;
  windowId?: number;
  windowBounds?: AreaRect;
  onCaptured?: () => void | Promise<void>;
}

interface AreaRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

function captureRegion(
  rect: AreaRect,
  screenshotPath: string,
  onFailure: CaptureFailureHandler,
  options?: CaptureAreaOptions
): Promise<boolean> {
  if (options?.windowId !== undefined && !options.cached) {
    return captureWindowByIdToFile(options.windowId, screenshotPath, onFailure);
  }

  if (
    options?.windowId !== undefined &&
    options.cached &&
    options.windowBounds
  ) {
    return captureFrozenWindowToFile(
      options.windowBounds,
      screenshotPath,
      options.windowId,
      onFailure
    );
  }

  if (options?.cached === undefined) {
    return captureRegionToFile(rect, screenshotPath, { onFailure });
  }

  return captureRegionToFile(rect, screenshotPath, {
    cached: options.cached,
    ...(options.windowId === undefined ? {} : { windowId: options.windowId }),
    onFailure,
  });
}

export async function captureArea(
  area: AreaSelection,
  options?: CaptureAreaOptions
): Promise<string | null> {
  if (
    area.x === undefined ||
    area.y === undefined ||
    area.width === undefined ||
    area.height === undefined
  ) {
    console.error('Invalid area selection');
    return null;
  }

  const rect: AreaRect = {
    x: area.x,
    y: area.y,
    width: area.width,
    height: area.height,
  };

  const preparation: { current: ReturnType<typeof prepareScreenshotPreview> } =
    {
      current: null,
    };

  try {
    const config = getConfig();
    const shouldHideIcons =
      config.screenshot.hideDesktopIcons &&
      isDesktopIconsSupported() &&
      checkAccessibilityPermission(false);

    if (shouldHideIcons) {
      await hideDesktopIcons('capture');
    }

    let restoreIcons: Promise<boolean> | null = null;
    try {
      const screenshotPath = await captureScreenshotToUniqueFile(
        async (temporaryPath, started, failed) => {
          const path = temporaryPath();
          const capture = captureRegion(rect, path, failed, options);
          started();
          preparation.current = prepareScreenshotPreview();
          const captured = await capture;
          restoreIcons = shouldHideIcons ? showDesktopIcons('capture') : null;
          if (!captured || !fs.existsSync(path)) return false;
          await options?.onCaptured?.();
          return true;
        }
      );
      if (!screenshotPath) {
        return null;
      }

      await finalizeCapture(screenshotPath, preparation.current);

      return screenshotPath;
    } finally {
      if (shouldHideIcons) {
        await (restoreIcons ?? showDesktopIcons('capture'));
      }
    }
  } finally {
    preparation.current?.dispose();
  }
}
