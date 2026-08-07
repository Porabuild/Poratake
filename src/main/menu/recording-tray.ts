import { app, Tray, nativeImage } from 'electron';
import path from 'path';
import { isProduction } from '@/main/utils/env.ts';
import { getPublicAssetPath } from '@/main/utils/paths.ts';
import { isWindows } from '@/main/utils/platform.ts';
import { stopRecordingAction } from '@/main/capture/video';
import { rebuildTrayMenu } from './index.ts';

let recordingTray: Tray | null = null;

function getRecordingTrayIconPath(): string {
  if (isWindows) {
    return getPublicAssetPath('icon.png');
  }
  if (isProduction) {
    return path.join(
      process.resourcesPath,
      'menu-icons',
      'recording',
      'iconTemplate.png'
    );
  }
  return path.join(
    app.getAppPath(),
    'src/main/menu/recording/iconTemplate.png'
  );
}

async function handleStopRecording(): Promise<void> {
  await stopRecordingAction();
  hideRecordingTray();
  rebuildTrayMenu();
}

export function showRecordingTray(): void {
  if (recordingTray) {
    return;
  }

  const iconPath = getRecordingTrayIconPath();
  const icon = nativeImage.createFromPath(iconPath);
  const trayIcon =
    isWindows && !icon.isEmpty()
      ? icon.resize({ width: 16, height: 16 })
      : icon;

  recordingTray = new Tray(trayIcon);
  recordingTray.setToolTip('Click to stop recording');
  recordingTray.setIgnoreDoubleClickEvents(true);

  recordingTray.on('click', handleStopRecording);
  recordingTray.on('right-click', handleStopRecording);
}

export function hideRecordingTray(): void {
  if (recordingTray) {
    recordingTray.destroy();
    recordingTray = null;
  }
}
