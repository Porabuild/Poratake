import { randomUUID } from 'crypto';
import { app, clipboard, Notification } from 'electron';
import fs from 'fs';
import path from 'path';
import { getConfig } from '@/main/settings';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
} from '@/main/capture/desktop-icons';
import { daemon } from '@/main/daemon';
import { isFeatureSupported } from '@/main/system/capabilities';
import { preprocessImageForOcr } from '@/main/utils/ffmpeg';
import { isMac } from '@/main/utils/platform';
import { captureAreaToFile } from '@/main/capture/area-overlay';
import { captureRegionToFile } from '@/main/capture/screenshot/native-capture';
import {
  runScreencapture,
  startInteractiveScreencapture,
} from '@/main/capture/screenshot/screencapture';

let isCapturingText = false;

interface CaptureTextArea {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface CaptureTextOptions {
  cached?: boolean;
  onCaptured?: () => void | Promise<void>;
}

export default async function captureText(
  area?: CaptureTextArea,
  options?: CaptureTextOptions
): Promise<void> {
  if (!isFeatureSupported('ocr') || isCapturingText) {
    return;
  }

  isCapturingText = true;

  try {
    await captureAndRecognizeText(area, options);
  } finally {
    isCapturingText = false;
  }
}

async function captureAndRecognizeText(
  area?: CaptureTextArea,
  options?: CaptureTextOptions
): Promise<void> {
  const config = getConfig();
  const shouldHideIcons =
    config.screenshot.hideDesktopIcons && isDesktopIconsSupported();

  if (shouldHideIcons) {
    await hideDesktopIcons('capture');
  }

  const tempDir = app.getPath('temp');
  const tempScreenshotPath = path.join(
    tempDir,
    `capty-ocr-${randomUUID()}.png`
  );
  const tempProcessedPath = path.join(
    tempDir,
    `capty-ocr-processed-${randomUUID()}.png`
  );

  try {
    let captured = false;
    try {
      if (!isMac) {
        try {
          if (!area) {
            captured = await captureAreaToFile(tempScreenshotPath);
          } else if (options?.cached === undefined) {
            captured = await captureRegionToFile(area, tempScreenshotPath);
          } else {
            captured = await captureRegionToFile(area, tempScreenshotPath, {
              cached: options.cached,
            });
          }
        } catch (error) {
          console.error('OCR capture error:', error);
          showNotification('OCR Failed', 'Failed to capture the selected area');
        }
      } else {
        try {
          const args = area
            ? [
                '-x',
                '-R',
                `${area.x},${area.y},${area.width},${area.height}`,
                '-t',
                'png',
                tempScreenshotPath,
              ]
            : ['-i', '-x', '-t', 'png', tempScreenshotPath];
          const capture = area
            ? runScreencapture(args)
            : startInteractiveScreencapture(args);
          if (!capture) return;

          const stderr = await capture;
          if (stderr) {
            console.log(`Screencapture stderr: ${stderr}`);
          }
          captured = fs.existsSync(tempScreenshotPath);
        } catch (error) {
          const message =
            error instanceof Error ? error.message : String(error);
          console.log(`Screencapture error: ${message}`);
        }
      }
    } finally {
      if (shouldHideIcons) {
        await showDesktopIcons('capture');
      }
    }

    if (!captured) return;

    await options?.onCaptured?.();

    const processed =
      !isMac &&
      (await preprocessImageForOcr(tempScreenshotPath, tempProcessedPath));
    await recognizeAndCopy(processed ? tempProcessedPath : tempScreenshotPath);
  } finally {
    deleteTemporaryScreenshot(tempScreenshotPath);
    deleteTemporaryScreenshot(tempProcessedPath);
  }
}

async function recognizeAndCopy(imagePath: string): Promise<void> {
  try {
    const extractedText = await extractTextFromImage(imagePath);

    if (extractedText && extractedText.trim()) {
      const trimmedText = extractedText.trim();
      clipboard.writeText(trimmedText);
      showNotification(
        'Text copied!',
        'Recognized text has been copied to the clipboard'
      );
    } else {
      showNotification(
        'No Text Found',
        'No text was detected in the selected area'
      );
    }
  } catch (err) {
    console.error('OCR error:', err);
    showNotification('OCR Failed', 'Failed to extract text from the image');
  }
}

function deleteTemporaryScreenshot(imagePath: string): void {
  try {
    if (fs.existsSync(imagePath)) {
      fs.unlinkSync(imagePath);
    }
  } catch (error) {
    console.error('Failed to delete OCR screenshot:', error);
  }
}

async function extractTextFromImage(imagePath: string): Promise<string> {
  const result = await daemon.call<{ text: string }>('ocr', 'recognize', {
    imagePath,
  });
  return result?.text || '';
}

function showNotification(title: string, body: string): void {
  const notification = new Notification({
    title,
    body,
  });
  notification.show();
}
