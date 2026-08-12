import { ipcMain, type WebContents } from 'electron';
import { daemon } from '@/main/daemon';
import { getConfig } from '@/main/settings';
import { isSettingsWindowWebContents } from '@/main/settings/window';
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

interface MicTestSession {
  sender: WebContents;
  active: boolean;
  nativeMayBeRunning: boolean;
}

interface CameraTestSession {
  sender: WebContents;
  nativeMayBeVisible: boolean;
  restorePreview: boolean;
}

let micTestSession: MicTestSession | null = null;
let cameraTestSession: CameraTestSession | null = null;

export async function listMediaDevices(): Promise<MediaDeviceLists> {
  const result = await daemon.call<Partial<MediaDeviceLists>>(
    'media-devices',
    'list'
  );
  return {
    microphones: result?.microphones ?? [],
    cameras: result?.cameras ?? [],
    defaultMicrophoneId: result?.defaultMicrophoneId ?? null,
    defaultCameraId: result?.defaultCameraId ?? null,
  };
}

function forwardMicLevel(event: string, data?: unknown): void {
  if (event !== 'media-devices:mic-level') return;
  if (!micTestSession?.active || micTestSession.sender.isDestroyed()) {
    return;
  }

  const level = (data as { level?: number } | undefined)?.level ?? 0;
  micTestSession.sender.send('devices:mic-test:level', level);
}

async function stopMicTest(sender?: WebContents): Promise<void> {
  const session = micTestSession;
  if (!session || (sender && session.sender !== sender)) return;

  micTestSession = null;
  if (!session.nativeMayBeRunning) return;
  await daemon.call('media-devices', 'stopMicTest').catch(error => {
    console.error('Failed to stop mic test:', error);
  });
}

async function stopCameraTest(sender?: WebContents): Promise<void> {
  const session = cameraTestSession;
  if (!session || (sender && session.sender !== sender)) return;

  cameraTestSession = null;
  const camera = getConfig().recording.camera;
  if (session.restorePreview && camera?.enabled) {
    await showCameraPreview(camera).catch(() => hideCameraPreview());
    return;
  }

  if (session.nativeMayBeVisible) {
    hideCameraPreview();
  }
}

const watchedSenders = new WeakSet<WebContents>();

function watchSender(sender: WebContents): void {
  if (watchedSenders.has(sender)) return;

  watchedSenders.add(sender);
  sender.once('destroyed', () => {
    if (micTestSession?.sender === sender) {
      void stopMicTest(sender);
    }
    if (cameraTestSession?.sender === sender) {
      void stopCameraTest(sender);
    }
  });
}

export function init(): void {
  daemon.onEvent(forwardMicLevel);

  ipcMain.handle('devices:list', async (event): Promise<MediaDeviceLists> => {
    if (!isSettingsWindowWebContents(event.sender)) {
      return {
        microphones: [],
        cameras: [],
        defaultMicrophoneId: null,
        defaultCameraId: null,
      };
    }

    return listMediaDevices();
  });

  ipcMain.handle(
    'devices:mic-test:start',
    async (event, target: DeviceTestTarget): Promise<boolean> => {
      if (!isSettingsWindowWebContents(event.sender)) return false;

      const session: MicTestSession = {
        sender: event.sender,
        active: false,
        nativeMayBeRunning: micTestSession?.nativeMayBeRunning ?? false,
      };
      micTestSession = session;
      watchSender(event.sender);

      const hasPermission = await checkAndRequestMicrophonePermission();
      if (
        !hasPermission ||
        micTestSession !== session ||
        event.sender.isDestroyed()
      ) {
        if (micTestSession === session) {
          await stopMicTest(event.sender);
        }
        return false;
      }

      session.nativeMayBeRunning = true;
      try {
        await daemon.call('media-devices', 'startMicTest', {
          deviceId: target?.deviceId ?? null,
          deviceName: target?.deviceName ?? null,
        });
      } catch (error) {
        if (micTestSession === session) {
          await stopMicTest(event.sender);
        }
        throw error;
      }

      if (micTestSession !== session || event.sender.isDestroyed()) {
        return false;
      }

      session.active = true;
      return true;
    }
  );

  ipcMain.handle('devices:mic-test:stop', async event => {
    if (!isSettingsWindowWebContents(event.sender)) return false;
    await stopMicTest(event.sender);
    return true;
  });

  ipcMain.handle(
    'devices:camera-test:start',
    async (event, target: DeviceTestTarget): Promise<boolean> => {
      if (!isSettingsWindowWebContents(event.sender)) return false;

      const previewWasVisible = cameraTestSession
        ? false
        : isCameraPreviewVisible();
      const session: CameraTestSession = {
        sender: event.sender,
        nativeMayBeVisible:
          cameraTestSession?.nativeMayBeVisible ?? previewWasVisible,
        restorePreview: cameraTestSession?.restorePreview ?? previewWasVisible,
      };
      cameraTestSession = session;
      watchSender(event.sender);

      const hasPermission = await checkAndRequestCameraPermission();
      if (
        !hasPermission ||
        cameraTestSession !== session ||
        event.sender.isDestroyed()
      ) {
        if (cameraTestSession === session) {
          await stopCameraTest(event.sender);
        }
        return false;
      }

      const camera = getConfig().recording.camera;
      session.nativeMayBeVisible = true;
      try {
        await showCameraPreview({
          ...camera,
          selectedDeviceId: target?.deviceId ?? null,
          selectedDeviceName: target?.deviceName ?? null,
          flipped: target?.flipped ?? camera.flipped,
        });
      } catch (error) {
        if (cameraTestSession === session) {
          await stopCameraTest(event.sender);
        }
        throw error;
      }

      if (cameraTestSession !== session || event.sender.isDestroyed()) {
        return false;
      }

      return true;
    }
  );

  ipcMain.handle('devices:camera-test:stop', async event => {
    if (!isSettingsWindowWebContents(event.sender)) return false;
    await stopCameraTest(event.sender);
    return true;
  });
}
