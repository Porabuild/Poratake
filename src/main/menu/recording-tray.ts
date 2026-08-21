import { app, nativeTheme, Tray } from 'electron';
import path from 'path';
import { isProduction } from '@/main/utils/env.ts';
import { getPublicAssetPath } from '@/main/utils/paths.ts';
import { isWindows } from '@/main/utils/platform.ts';
import { stopRecordingAction } from '@/main/capture/video';
import { rebuildTrayMenu } from './index.ts';
import { createTrayIcon } from './tray-icon.ts';

let recordingTray: Tray | null = null;

function updateRecordingTrayIcon(): void {
  recordingTray?.setImage(createTrayIcon(getRecordingTrayIconPath()));
}

function getRecordingTrayIconPath(): string {
  if (isWindows) {
    return getPublicAssetPath('tray-icon.png');
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
  try {
    await stopRecordingAction();
  } catch (error) {
    console.error('Error stopping recording from tray:', error);
  } finally {
    hideRecordingTray();
    rebuildTrayMenu();
  }
}

export function showRecordingTray(): void {
  if (recordingTray) {
    return;
  }

  recordingTray = new Tray(createTrayIcon(getRecordingTrayIconPath()));
  recordingTray.setToolTip('Click to stop recording');
  recordingTray.setIgnoreDoubleClickEvents(true);

  recordingTray.on('click', handleStopRecording);
  recordingTray.on('right-click', handleStopRecording);
  nativeTheme.on('updated', updateRecordingTrayIcon);
}

export function hideRecordingTray(): void {
  if (recordingTray) {
    nativeTheme.off('updated', updateRecordingTrayIcon);
    recordingTray.destroy();
    recordingTray = null;
  }
}
