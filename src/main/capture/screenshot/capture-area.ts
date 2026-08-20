import fs from 'fs';
import { getConfig } from '@/main/settings';
import { generateScreenshotPath } from './utils.ts';
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
  options?: CaptureAreaOptions
): Promise<boolean> {
  if (options?.windowId !== undefined && !options.cached) {
    return captureWindowByIdToFile(options.windowId, screenshotPath);
  }

  if (
    options?.windowId !== undefined &&
    options.cached &&
    options.windowBounds
  ) {
    return captureFrozenWindowToFile(
      options.windowBounds,
      screenshotPath,
      options.windowId
    );
  }

  if (options?.cached === undefined) {
    return captureRegionToFile(rect, screenshotPath);
  }

  return captureRegionToFile(rect, screenshotPath, {
    cached: options.cached,
    ...(options.windowId === undefined ? {} : { windowId: options.windowId }),
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

  let preparation: ReturnType<typeof prepareScreenshotPreview> = null;

  try {
    const config = getConfig();
    const screenshotPath = generateScreenshotPath();
    const shouldHideIcons =
      config.screenshot.hideDesktopIcons &&
      isDesktopIconsSupported() &&
      checkAccessibilityPermission(false);

    if (shouldHideIcons) {
      await hideDesktopIcons('capture');
    }

    let restoreIcons: Promise<boolean> | null = null;
    let captured: boolean;
    try {
      const capture = captureRegion(rect, screenshotPath, options);
      preparation = prepareScreenshotPreview();
      captured = await capture;
      restoreIcons = shouldHideIcons ? showDesktopIcons('capture') : null;
    } catch (error) {
      if (shouldHideIcons) {
        await showDesktopIcons('capture');
      }
      throw error;
    }

    if (!captured) {
      await restoreIcons;
      return null;
    }

    try {
      if (!fs.existsSync(screenshotPath)) {
        return null;
      }

      await options?.onCaptured?.();
      await finalizeCapture(screenshotPath, preparation);

      return screenshotPath;
    } finally {
      await restoreIcons;
    }
  } finally {
    preparation?.dispose();
  }
}
