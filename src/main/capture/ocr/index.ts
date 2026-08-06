import { exec } from 'child_process';
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

export default async function captureText(): Promise<void> {
  const config = getConfig();
  const shouldHideIcons =
    config.screenshot.hideDesktopIcons && isDesktopIconsSupported();

  if (shouldHideIcons) {
    await hideDesktopIcons('capture');
  }

  const timestamp = Date.now();
  const tempDir = app.getPath('temp');
  const tempScreenshotPath = path.join(tempDir, `capty-ocr-${timestamp}.png`);

  const command = `screencapture -i -x -t png "${tempScreenshotPath}"`;

  exec(command, async (error, _stdout, stderr) => {
    if (shouldHideIcons) {
      await showDesktopIcons('capture');
    }

    if (error) {
      console.log(`Screencapture error: ${error.message}`);
      return;
    }
    if (stderr) {
      console.log(`Screencapture stderr: ${stderr}`);
      return;
    }

    if (!fs.existsSync(tempScreenshotPath)) {
      return;
    }

    try {
      const extractedText = await extractTextFromImage(tempScreenshotPath);

      fs.unlinkSync(tempScreenshotPath);

      if (extractedText && extractedText.trim()) {
        const trimmedText = extractedText.trim();
        clipboard.writeText(trimmedText);
        showNotification('Text copied!', trimmedText);
      } else {
        showNotification(
          'No Text Found',
          'No text was detected in the selected area'
        );
      }
    } catch (err) {
      console.error('OCR error:', err);
      if (fs.existsSync(tempScreenshotPath)) {
        fs.unlinkSync(tempScreenshotPath);
      }
      showNotification('OCR Failed', 'Failed to extract text from the image');
    }
  });
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
