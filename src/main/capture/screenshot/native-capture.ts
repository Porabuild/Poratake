import { screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import { daemon } from '@/main/daemon';

export interface RegionCaptureOptions {
  frozen?: boolean;
}

export async function captureRegionToFile(
  area: Rectangle,
  filePath: string,
  options?: RegionCaptureOptions
): Promise<boolean> {
  const bounds = screen.dipToScreenRect(null, area);

  try {
    await daemon.call('screenshot', 'capture-area', {
      x: bounds.x,
      y: bounds.y,
      width: bounds.width,
      height: bounds.height,
      path: filePath,
      frozen: options?.frozen ?? false,
    });
    return true;
  } catch (error) {
    console.error('Area capture failed:', error);
    return false;
  }
}

export function captureDisplayToFile(
  display: Display,
  filePath: string
): Promise<boolean> {
  return captureRegionToFile(display.bounds, filePath);
}

export async function captureWindowToFile(
  windowId: number,
  filePath: string
): Promise<boolean> {
  try {
    await daemon.call('screenshot', 'capture-window', {
      windowId,
      path: filePath,
    });
    return true;
  } catch (error) {
    console.error('Window capture failed:', error);
    return false;
  }
}
