import { screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';
import { DAEMON_UNAVAILABLE } from '@/types/daemon';

export type CaptureFailureHandler = (completed: boolean) => void;

export interface RegionCaptureOptions {
  cached?: boolean;
  windowId?: number;
  onFailure?: CaptureFailureHandler;
}

function captureCompleted(error: unknown): boolean {
  const code = (error as { code?: string } | null)?.code;
  return (
    code === 'CAPTURE_FAILED' ||
    code === 'INVALID_PARAMS' ||
    code === 'METHOD_NOT_FOUND' ||
    code === DAEMON_UNAVAILABLE
  );
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
    options?.onFailure?.(captureCompleted(error));
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
  filePath: string,
  onFailure?: CaptureFailureHandler
): Promise<boolean> {
  return captureRegionToFile(
    display.bounds,
    filePath,
    onFailure ? { onFailure } : undefined
  );
}

export function captureFrozenWindowToFile(
  bounds: Rectangle,
  filePath: string,
  windowId: number,
  onFailure?: CaptureFailureHandler
): Promise<boolean> {
  return capturePhysicalRegionToFile(bounds, filePath, {
    cached: true,
    windowId,
    ...(onFailure ? { onFailure } : {}),
  });
}

export async function captureWindowByIdToFile(
  windowId: number,
  filePath: string,
  onFailure?: CaptureFailureHandler
): Promise<boolean> {
  try {
    await daemon.call('screenshot', 'capture-window', {
      windowId,
      path: filePath,
    });
    return true;
  } catch (error) {
    onFailure?.(captureCompleted(error));
    console.error('Window capture failed:', error);
    return false;
  }
}
