import { ipcMain, screen } from 'electron';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';
import type { CameraSettings } from '@/types/settings';

let currentSettings: CameraSettings | null = null;
let isContentProtected = false;

function toNativePosition(position: { x: number; y: number }): {
  x: number;
  y: number;
} {
  return isWindows ? screen.dipToScreenPoint(position) : position;
}

function fromNativePosition(position: { x: number; y: number }): {
  x: number;
  y: number;
} {
  return isWindows ? screen.screenToDipPoint(position) : position;
}

export async function showCameraPreview(
  settings: CameraSettings
): Promise<void> {
  const position = settings.position
    ? toNativePosition(settings.position)
    : undefined;

  try {
    await daemon.call('camera-preview', 'show', {
      deviceId: settings.selectedDeviceId,
      deviceName: settings.selectedDeviceName,
      resolution: settings.resolution || '720p',
      x: position?.x,
      y: position?.y,
    });

    if (isContentProtected) {
      await daemon.call('camera-preview', 'setContentProtection', {
        enabled: true,
      });
    }
    currentSettings = settings;
  } catch (error) {
    currentSettings = null;
    await daemon.call('camera-preview', 'hide').catch(() => {});
    throw error;
  }
}

export function hideCameraPreview(): void {
  daemon.call('camera-preview', 'hide').catch(error => {
    console.error('Failed to hide camera preview:', error);
  });

  currentSettings = null;
}

export function updateCameraPreview(settings: CameraSettings): void {
  currentSettings = settings;

  const params: Record<string, unknown> = {
    deviceId: settings.selectedDeviceId,
    deviceName: settings.selectedDeviceName,
    resolution: settings.resolution,
  };

  if (settings.position) {
    const position = toNativePosition(settings.position);
    params.x = position.x;
    params.y = position.y;
  }

  daemon.call('camera-preview', 'update', params).catch(error => {
    console.error('Failed to update camera preview:', error);
  });
}

export function getCameraPreviewWindow(): null {
  return null;
}

export function isCameraPreviewVisible(): boolean {
  return currentSettings !== null;
}

export async function getCameraPreviewPosition(): Promise<{
  x: number;
  y: number;
} | null> {
  try {
    const result = (await daemon.call('camera-preview', 'getPosition')) as {
      x?: number;
      y?: number;
    } | null;
    if (
      result &&
      typeof result.x === 'number' &&
      typeof result.y === 'number'
    ) {
      return fromNativePosition({ x: result.x, y: result.y });
    }
    return null;
  } catch {
    return null;
  }
}

export async function enableCameraContentProtection(): Promise<void> {
  isContentProtected = true;
  try {
    await daemon.call('camera-preview', 'setContentProtection', {
      enabled: true,
    });
  } catch (error) {
    isContentProtected = false;
    throw error;
  }
}

export async function disableCameraContentProtection(): Promise<void> {
  isContentProtected = false;
  try {
    await daemon.call('camera-preview', 'setContentProtection', {
      enabled: false,
    });
  } catch (error) {
    console.error('Failed to disable camera content protection:', error);
  }
}

export function isCameraContentProtectionEnabled(): boolean {
  return isContentProtected;
}

export function registerCameraPreviewIpcHandlers(): void {
  const positionHandler = (event: string, data: unknown) => {
    if (
      event === 'camera-preview:position-changed' &&
      currentSettings &&
      data
    ) {
      const pos = data as { x?: number; y?: number };
      currentSettings.position = fromNativePosition({
        x: pos.x ?? 0,
        y: pos.y ?? 0,
      });
    }
  };

  daemon.onEvent(positionHandler);

  ipcMain.handle('camera:get-settings', () => {
    return currentSettings;
  });
}
