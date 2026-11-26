import { ipcMain, screen } from 'electron';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';
import type { CameraSettings } from '@/types/settings';

interface CameraPreviewPositionResult {
  position?: { x: number; y: number };
}

let currentSettings: CameraSettings | null = null;
let isContentProtected = false;
let ipcHandlersRegistered = false;
let previewRequestGeneration = 0;

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
  const requestGeneration = ++previewRequestGeneration;
  const position = settings.position
    ? toNativePosition(settings.position)
    : undefined;

  try {
    const result = await daemon.call<CameraPreviewPositionResult | null>(
      'camera-preview',
      'show',
      {
        deviceId: settings.selectedDeviceId,
        deviceName: settings.selectedDeviceName,
        resolution: settings.resolution || '720p',
        flipped: settings.flipped ?? false,
        x: position?.x,
        y: position?.y,
      }
    );
    if (requestGeneration !== previewRequestGeneration) return;

    if (isContentProtected) {
      await daemon.call('camera-preview', 'setContentProtection', {
        enabled: true,
      });
      if (requestGeneration !== previewRequestGeneration) return;
    }
    currentSettings = { ...settings };
    if (result?.position) {
      currentSettings.position = fromNativePosition(result.position);
    }
  } catch (error) {
    if (requestGeneration !== previewRequestGeneration) return;
    currentSettings = null;
    await daemon.call('camera-preview', 'hide').catch(() => {});
    throw error;
  }
}

export function hideCameraPreview(): void {
  previewRequestGeneration += 1;
  daemon.call('camera-preview', 'hide').catch(error => {
    console.error('Failed to hide camera preview:', error);
  });

  currentSettings = null;
}

export async function updateCameraPreviewPosition(position: {
  x: number;
  y: number;
}): Promise<void> {
  const settings = currentSettings;
  if (!settings) return;

  const result = await daemon.call<CameraPreviewPositionResult | null>(
    'camera-preview',
    'update',
    toNativePosition(position)
  );
  if (currentSettings !== settings) return;
  settings.position = result?.position
    ? fromNativePosition(result.position)
    : position;
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
