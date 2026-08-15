import { screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';

export interface RegionCaptureOptions {
  cached?: boolean;
  windowId?: number;
}

async function capturePhysicalRegionToFile(
  area: Rectangle,
  filePath: string,
  options?: RegionCaptureOptions
): Promise<boolean> {
  try {
    await daemon.call('screenshot', 'capture-area', {
      x: area.x,
      y: area.y,
      width: area.width,
      height: area.height,
      path: filePath,
      cached: options?.cached ?? false,
      ...(options?.windowId === undefined
        ? {}
        : { windowId: options.windowId }),
    });
    return true;
  } catch (error) {
    console.error('Area capture failed:', error);
    return false;
  }
}

export function captureRegionToFile(
  area: Rectangle,
  filePath: string,
  options?: RegionCaptureOptions
): Promise<boolean> {
  const captureArea = isWindows ? screen.dipToScreenRect(null, area) : area;
  return capturePhysicalRegionToFile(captureArea, filePath, options);
}

export function captureDisplayToFile(
  display: Display,
  filePath: string
): Promise<boolean> {
  return captureRegionToFile(display.bounds, filePath);
}

export function captureFrozenWindowToFile(
  bounds: Rectangle,
  filePath: string,
  windowId: number
): Promise<boolean> {
  return capturePhysicalRegionToFile(bounds, filePath, {
    cached: true,
    windowId,
  });
}

export async function captureWindowByIdToFile(
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
