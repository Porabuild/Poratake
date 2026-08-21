import { nativeImage, nativeTheme } from 'electron';
import type { NativeImage } from 'electron';
import { isWindows } from '@/main/utils/platform';

const TRAY_ICON_SIZE = 16;
const MONOCHROME_TOLERANCE = 24;

function adaptMonochromePixels(icon: NativeImage, dark: boolean): NativeImage {
  const { width, height } = icon.getSize();
  const bitmap = icon.toBitmap();
  const value = dark ? 255 : 0;

  for (let i = 0; i < bitmap.length; i += 4) {
    const red = bitmap[i];
    const green = bitmap[i + 1];
    const blue = bitmap[i + 2];
    const spread = Math.max(red, green, blue) - Math.min(red, green, blue);
    if (spread > MONOCHROME_TOLERANCE) {
      continue;
    }
    bitmap[i] = value;
    bitmap[i + 1] = value;
    bitmap[i + 2] = value;
  }

  return nativeImage.createFromBuffer(bitmap, { width, height });
}

export function createTrayIcon(iconPath: string): NativeImage {
  const icon = nativeImage.createFromPath(iconPath);
  if (!isWindows || icon.isEmpty()) {
    return icon;
  }

  const resized = icon.resize({
    width: TRAY_ICON_SIZE,
    height: TRAY_ICON_SIZE,
  });
  return adaptMonochromePixels(resized, nativeTheme.shouldUseDarkColors);
}
