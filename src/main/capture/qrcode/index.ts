import { randomUUID } from 'crypto';
import { app, clipboard } from 'electron';
import fs from 'fs';
import path from 'path';
import {
  hideDesktopIcons,
  showDesktopIcons,
} from '@/main/capture/desktop-icons';
import { shouldHideDesktopIconsForCapture } from '@/main/capture/desktop-icons/preference';
import { daemon } from '@/main/daemon';
import { isFeatureSupported } from '@/main/system/capabilities';
import { showTransientNotification } from '@/main/utils/notification';
import { captureAreaToFile } from '@/main/capture/area-overlay';
import { showNotification } from '@/main/utils/notification';

let isScanningQRCode = false;

export default async function scanQRCode(): Promise<void> {
  if (!isFeatureSupported('qrcode') || isScanningQRCode) {
    return;
  }

  isScanningQRCode = true;

  try {
    await captureAndDecodeQRCode();
  } finally {
    isScanningQRCode = false;
  }
}

async function captureAndDecodeQRCode(): Promise<void> {
  const shouldHideIcons = shouldHideDesktopIconsForCapture();

  if (shouldHideIcons) {
    await hideDesktopIcons('capture');
  }

  const tempDir = app.getPath('temp');
  const tempScreenshotPath = path.join(
    tempDir,
    `poratake-qrcode-${randomUUID()}.png`
  );

  try {
    let captured = false;
    try {
      captured = await captureAreaToFile(tempScreenshotPath);
    } catch (error) {
      console.error('QR code capture error:', error);
      showNotification('Scan Failed', 'Failed to capture the selected area');
    } finally {
      if (shouldHideIcons) {
        await showDesktopIcons('capture');
      }
    }

    if (!captured) return;

    await decodeAndCopy(tempScreenshotPath);
  } finally {
    deleteTemporaryScreenshot(tempScreenshotPath);
  }
}

async function decodeAndCopy(imagePath: string): Promise<void> {
  try {
    const qrCodeValue = await extractQRCode(imagePath);

    if (qrCodeValue && qrCodeValue.trim()) {
      clipboard.writeText(qrCodeValue.trim());
      showTransientNotification(
        'QR Code Copied',
        'QR code value has been copied to clipboard'
      );
      return;
    }

    showNotification(
      'No QR Code Found',
      'No QR code was detected in the selected area'
    );
  } catch (err) {
    console.error('QR code scan error:', err);
    showNotification('Scan Failed', 'Failed to scan QR code from the image');
  }
}

function deleteTemporaryScreenshot(imagePath: string): void {
  try {
    if (fs.existsSync(imagePath)) {
      fs.unlinkSync(imagePath);
    }
  } catch (error) {
    console.error('Failed to delete QR code screenshot:', error);
  }
}

async function extractQRCode(imagePath: string): Promise<string> {
  const result = await daemon.call<{ payload: string }>('qrcode', 'detect', {
    imagePath,
  });
  return result?.payload || '';
}
