import { ipcMain } from 'electron';
import {
  showCameraPreview,
  hideCameraPreview,
  updateCameraPreview,
  getCameraPreviewPosition,
} from './camera-preview.ts';
import { getConfig, updateConfig } from '@/main/settings';
import { daemon } from '@/main/daemon';
import type { RecordingSettings } from '@/types/settings.ts';

export function registerSettingsIpcHandlers(): void {
  ipcMain.handle('recording-settings:get', () => {
    return getConfig().recording;
  });

  ipcMain.on(
    'recording-settings:update',
    async (_event, settings: Partial<RecordingSettings>) => {
      const currentPosition = await getCameraPreviewPosition();

      if (settings.camera && currentPosition) {
        settings.camera.position = currentPosition;
      }

      updateConfig({ recording: settings as RecordingSettings });

      const fullSettings = getConfig().recording;

      if (fullSettings.camera) {
        if (fullSettings.camera.enabled) {
          showCameraPreview(fullSettings.camera);
        } else {
          hideCameraPreview();
        }
      }

      daemon
        .call('recording-control', 'updateSettings', {
          systemAudio: fullSettings.systemAudio,
          micEnabled: fullSettings.micEnabled,
          cameraEnabled: fullSettings.camera?.enabled ?? false,
        })
        .catch(() => {});
    }
  );

  ipcMain.on(
    'camera:position-update',
    (_event, position: { x: number; y: number }) => {
      const config = getConfig();
      if (config.recording.camera) {
        config.recording.camera.position = position;
        updateConfig({ recording: config.recording });
      }
    }
  );

  ipcMain.on(
    'camera:update-settings',
    (_event, settings: Partial<RecordingSettings['camera']>) => {
      const config = getConfig();
      if (config.recording.camera) {
        const updatedCamera = { ...config.recording.camera, ...settings };
        updateConfig({
          recording: { ...config.recording, camera: updatedCamera },
        });
        updateCameraPreview(updatedCamera);
      }
    }
  );
}
