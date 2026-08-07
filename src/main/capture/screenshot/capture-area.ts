import { exec } from 'child_process';
import { clipboard, nativeImage } from 'electron';
import fs from 'fs';
import { getConfig } from '@/main/settings';
import { addToHistory } from '@/main/history';
import { generateScreenshotPath } from './utils.ts';
import type { AreaSelection } from '@/types/area.ts';
import { showCapturePreview } from '@/main/capture/capture-preview';
import { openScreenshotEditor } from '@/main/capture/screenshot/open-editor';
import { isMac } from '@/main/utils/platform';
import { captureRegionToFile } from '@/main/capture/screenshot/native-capture';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
} from '@/main/capture/desktop-icons';

export interface CaptureAreaOptions {
  onCaptured?: () => void | Promise<void>;
}

interface AreaRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

function captureRegionWithScreencapture(
  area: AreaRect,
  screenshotPath: string,
  disableSound: boolean
): Promise<void> {
  let command = 'screencapture';

  if (disableSound) {
    command += ' -x';
  }

  command += ` -R ${area.x},${area.y},${area.width},${area.height}`;
  command += ` -t png "${screenshotPath}"`;

  return new Promise((resolve, reject) => {
    exec(command, (error, _stdout, stderr) => {
      if (error) {
        console.error('Screenshot capture error:', error.message);
        reject(error);
        return;
      }

      if (stderr) {
        console.error('Screenshot capture stderr:', stderr);
      }

      resolve();
    });
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

  const config = getConfig();
  const screenshotPath = generateScreenshotPath();

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
    try {
      if (!(await captureRegionToFile(rect, screenshotPath))) {
        return null;
      }
    } finally {
      if (shouldHideIcons) {
        await showDesktopIcons('capture');
      }
    }
  }

  if (!fs.existsSync(screenshotPath)) {
    return null;
  }

  await options?.onCaptured?.();

  const historyItem = await addToHistory(screenshotPath);

  if (config.screenshot.captureToClipboard) {
    const imageBuffer = fs.readFileSync(screenshotPath);
    const image = nativeImage.createFromBuffer(imageBuffer);
    clipboard.writeImage(image);
    return screenshotPath;
  }

  if (config.screenshot.showPreview) {
    showCapturePreview(screenshotPath, 'screenshot', historyItem?.id);
    return screenshotPath;
  }

  openScreenshotEditor(screenshotPath, historyItem?.id);
  return screenshotPath;
}
