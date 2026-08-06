import { exec } from 'child_process';
import { clipboard, nativeImage } from 'electron';
import fs from 'fs';
import { getConfig } from '@/main/settings';
import { addToHistory } from '@/main/history';
import { generateScreenshotPath } from './utils.ts';
import type { AreaSelection } from '@/types/area.ts';
import { showCapturePreview } from '@/main/capture/capture-preview';
import { openScreenshotEditor } from '@/main/capture/screenshot/open-editor';

export interface CaptureAreaOptions {
  onCaptured?: () => void | Promise<void>;
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

  const config = getConfig();
  const screenshotPath = generateScreenshotPath();
  const disableSound = !config.general.playSoundOnScreenshot;

  let command = 'screencapture';

  if (disableSound) {
    command += ' -x';
  }

  command += ` -R ${area.x},${area.y},${area.width},${area.height}`;
  command += ` -t png "${screenshotPath}"`;

  return new Promise((resolve, reject) => {
    exec(command, async (error, _stdout, stderr) => {
      if (error) {
        console.error('Screenshot capture error:', error.message);
        reject(error);
        return;
      }

      if (stderr) {
        console.error('Screenshot capture stderr:', stderr);
      }

      if (!fs.existsSync(screenshotPath)) {
        resolve(null);
        return;
      }

      await options?.onCaptured?.();

      const historyItem = await addToHistory(screenshotPath);

      if (config.screenshot.captureToClipboard) {
        const imageBuffer = fs.readFileSync(screenshotPath);
        const image = nativeImage.createFromBuffer(imageBuffer);
        clipboard.writeImage(image);
        resolve(screenshotPath);
        return;
      }

      if (config.screenshot.showPreview) {
        showCapturePreview(screenshotPath, 'screenshot', historyItem?.id);
        resolve(screenshotPath);
        return;
      }

      openScreenshotEditor(screenshotPath, historyItem?.id);
      resolve(screenshotPath);
    });
  });
}
