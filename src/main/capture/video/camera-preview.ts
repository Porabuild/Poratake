import { ipcMain, screen } from 'electron';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';
import type { CameraSettings } from '@/types/settings';

let currentSettings: CameraSettings | null = null;
let isContentProtected = false;
let ipcHandlersRegistered = false;

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
      flipped: settings.flipped ?? false,
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

export async function updateCameraPreviewPosition(position: {
  x: number;
  y: number;
}): Promise<void> {
  if (!currentSettings) return;

  await daemon.call('camera-preview', 'update', toNativePosition(position));
  currentSettings = { ...currentSettings, position };
}

export function isCameraPreviewVisible(): boolean {
  return currentSettings !== null;
}

export function getCameraPreviewSettings(): CameraSettings | null {
  return currentSettings;
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
  if (ipcHandlersRegistered) {
    return;
  }
  ipcHandlersRegistered = true;
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

  ipcMain.handle('camera:get-settings', getCameraPreviewSettings);
}
