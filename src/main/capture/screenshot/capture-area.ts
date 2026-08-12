import fs from 'fs';
import { getConfig } from '@/main/settings';
import { generateScreenshotPath } from './utils.ts';
import type { AreaSelection } from '@/types/area.ts';
import {
  finalizeCapture,
  prepareScreenshotPreview,
} from '@/main/capture/screenshot/finalize';
import { isMac } from '@/main/utils/platform';
import {
  captureRegionToFile,
  captureWindowToFile,
} from '@/main/capture/screenshot/native-capture';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
} from '@/main/capture/desktop-icons';
import { runScreencapture } from '@/main/capture/screenshot/screencapture';

export interface CaptureAreaOptions {
  cached?: boolean;
  windowId?: number;
  onCaptured?: () => void | Promise<void>;
}

interface AreaRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

async function captureRegionWithScreencapture(
  area: AreaRect,
  screenshotPath: string,
  disableSound: boolean
): Promise<void> {
  const args: string[] = [];

  if (disableSound) {
    args.push('-x');
  }

  args.push(
    '-R',
    `${area.x},${area.y},${area.width},${area.height}`,
    '-t',
    'png',
    screenshotPath
  );

  try {
    const stderr = await runScreencapture(args);
    if (stderr) {
      console.error('Screenshot capture stderr:', stderr);
    }
  } catch (error) {
    console.error(
      'Screenshot capture error:',
      error instanceof Error ? error.message : String(error)
    );
    throw error;
  }
}

function captureRegion(
  rect: AreaRect,
  screenshotPath: string,
  options?: CaptureAreaOptions
): Promise<boolean> {
  if (options?.windowId !== undefined && !options.cached) {
    return captureWindowToFile(options.windowId, screenshotPath);
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

  const preparation = prepareScreenshotPreview();

  try {
    const config = getConfig();
    const screenshotPath = generateScreenshotPath();
    let restoreIcons: Promise<boolean> | null = null;

    if (isMac) {
      await captureRegionWithScreencapture(
        rect,
        screenshotPath,
        !config.general.playSoundOnScreenshot
      );
    } else {
      const shouldHideIcons =
        config.screenshot.hideDesktopIcons && isDesktopIconsSupported();
      if (shouldHideIcons) {
        await hideDesktopIcons('capture');
      }

      let captured: boolean;
      try {
        captured = await captureRegion(rect, screenshotPath, options);
      } catch (error) {
        if (shouldHideIcons) {
          await showDesktopIcons('capture');
        }
        throw error;
      }

      restoreIcons = shouldHideIcons ? showDesktopIcons('capture') : null;

      if (!captured) {
        await restoreIcons;
        return null;
      }
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
