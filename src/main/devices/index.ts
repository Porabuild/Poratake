import { ipcMain, type WebContents } from 'electron';
import { daemon } from '@/main/daemon';
import { getConfig } from '@/main/settings';
import {
  showCameraPreview,
  hideCameraPreview,
  isCameraPreviewVisible,
} from '@/main/capture/video/camera-preview';
import {
  checkAndRequestCameraPermission,
  checkAndRequestMicrophonePermission,
} from '@/main/capture/video/permissions';
import type { DeviceTestTarget, MediaDeviceLists } from '@/types/devices';

let micTestSender: WebContents | null = null;
let cameraTestActive = false;
let restorePreviewAfterTest = false;

function forwardMicLevel(event: string, data?: unknown): void {
  if (event !== 'media-devices:mic-level') return;
  if (!micTestSender || micTestSender.isDestroyed()) return;

  const level = (data as { level?: number } | undefined)?.level ?? 0;
  micTestSender.send('devices:mic-test:level', level);
}

async function stopMicTest(): Promise<void> {
  if (!micTestSender) return;

  micTestSender = null;
  await daemon.call('media-devices', 'stopMicTest').catch(error => {
    console.error('Failed to stop mic test:', error);
  });
}

async function stopCameraTest(): Promise<void> {
  if (!cameraTestActive) return;

  cameraTestActive = false;

  const camera = getConfig().recording.camera;
  if (restorePreviewAfterTest && camera?.enabled) {
    await showCameraPreview(camera).catch(() => hideCameraPreview());
    return;
  }

  hideCameraPreview();
}

const watchedSenders = new WeakSet<WebContents>();

function watchSender(sender: WebContents): void {
  if (watchedSenders.has(sender)) return;

  watchedSenders.add(sender);
  sender.once('destroyed', () => {
    if (micTestSender === sender) {
      void stopMicTest();
    }
    void stopCameraTest();
  });
}

export function init(): void {
  daemon.onEvent(forwardMicLevel);

  ipcMain.handle('devices:list', async (): Promise<MediaDeviceLists> => {
    const result = await daemon.call<Partial<MediaDeviceLists>>(
      'media-devices',
      'list'
    );
    return {
      microphones: result?.microphones ?? [],
      cameras: result?.cameras ?? [],
    };
  });

  ipcMain.handle(
    'devices:mic-test:start',
    async (event, target: DeviceTestTarget): Promise<boolean> => {
      const hasPermission = await checkAndRequestMicrophonePermission();
      if (!hasPermission) return false;

      await daemon.call('media-devices', 'startMicTest', {
        deviceId: target?.deviceId ?? null,
        deviceName: target?.deviceName ?? null,
      });

      micTestSender = event.sender;
      watchSender(event.sender);
      return true;
    }
  );

  ipcMain.handle('devices:mic-test:stop', () => stopMicTest());

  ipcMain.handle(
    'devices:camera-test:start',
    async (event, target: DeviceTestTarget): Promise<boolean> => {
      const hasPermission = await checkAndRequestCameraPermission();
      if (!hasPermission) return false;

      if (!cameraTestActive) {
        restorePreviewAfterTest = isCameraPreviewVisible();
      }

      const camera = getConfig().recording.camera;
      await showCameraPreview({
        ...camera,
        selectedDeviceId: target?.deviceId ?? null,
        selectedDeviceName: target?.deviceName ?? null,
        flipped: target?.flipped ?? camera.flipped,
      });

      cameraTestActive = true;
      watchSender(event.sender);
      return true;
    }
  );

  ipcMain.handle('devices:camera-test:stop', () => stopCameraTest());
}
