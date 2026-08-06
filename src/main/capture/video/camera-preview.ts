import { ipcMain } from 'electron';
import { daemon } from '@/main/daemon';
import type { CameraSettings } from '@/types/settings';

let currentSettings: CameraSettings | null = null;
let isContentProtected = false;

export function showCameraPreview(settings: CameraSettings): void {
  currentSettings = settings;

  daemon
    .call('camera-preview', 'show', {
      deviceId: settings.selectedDeviceId,
      deviceName: settings.selectedDeviceName,
      resolution: settings.resolution || '720p',
    })
    .catch(error => {
      console.error('Failed to show camera preview:', error);
    });

  if (isContentProtected) {
    daemon
      .call('camera-preview', 'setContentProtection', { enabled: true })
      .catch(() => {});
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
    params.x = settings.position.x;
    params.y = settings.position.y;
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
      return { x: result.x, y: result.y };
    }
    return null;
  } catch {
    return null;
  }
}

export function enableCameraContentProtection(): void {
  isContentProtected = true;
  daemon
    .call('camera-preview', 'setContentProtection', { enabled: true })
    .catch(() => {});
}

export function disableCameraContentProtection(): void {
  isContentProtected = false;
  daemon
    .call('camera-preview', 'setContentProtection', { enabled: false })
    .catch(() => {});
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
      currentSettings.position = { x: pos.x ?? 0, y: pos.y ?? 0 };
    }
  };

  daemon.onEvent(positionHandler);

  ipcMain.handle('camera:get-settings', () => {
    return currentSettings;
  });
}
