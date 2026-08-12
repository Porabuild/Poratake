import { dialog, BrowserWindow } from 'electron';
import {
  openScreenRecordingPreferences,
  getMicrophoneStatus,
  requestMicrophonePermission,
  openMicrophonePreferences,
  getCameraStatus,
  requestCameraPermission,
  openCameraPreferences,
} from '@/main/system/permissions.ts';
import { isMac, isWindows } from '@/main/utils/platform';

export async function showRecordingError(error: Error): Promise<void> {
  const isPermissionError =
    isMac &&
    (error.message.includes('permission') ||
      error.message.includes('TCC') ||
      error.message.includes('declined'));

  const win = BrowserWindow.getFocusedWindow();

  if (isPermissionError) {
    const options = {
      type: 'error' as const,
      title: 'Recording Failed',
      message: 'Screen recording permission issue detected.',
      detail:
        'After an app update, macOS may require you to re-grant screen recording permission.\n\n' +
        'To fix this:\n' +
        '1. Open System Settings > Privacy & Security > Screen Recording\n' +
        '2. Toggle OFF Poratake, then toggle it back ON\n' +
        '3. Restart Poratake\n\n' +
        'If the issue persists, try restarting your Mac.',
      buttons: ['Open Settings', 'OK'],
      defaultId: 0,
    };

    const result = win
      ? await dialog.showMessageBox(win, options)
      : await dialog.showMessageBox(options);

    if (result.response === 0) {
      openScreenRecordingPreferences();
    }
  } else {
    const options = {
      type: 'error' as const,
      title: 'Recording Failed',
      message: 'Recording failed.',
      detail: error.message,
      buttons: ['OK'],
    };

    if (win) {
      await dialog.showMessageBox(win, options);
    } else {
      await dialog.showMessageBox(options);
    }
  }
}

export async function checkAndRequestCameraPermission(): Promise<boolean> {
  const status = getCameraStatus();

  if (
    status === 'not-determined' ||
    status === 'denied' ||
    status === 'restricted'
  ) {
    const granted = await requestCameraPermission();
    if (!granted) {
      const options = {
        type: 'error' as const,
        title: 'Camera Permission Required',
        message: 'Camera access is not granted.',
        detail: isWindows
          ? 'To record with camera, please allow camera access in Windows Settings.\n\n' +
            'Go to: Settings > Privacy & security > Camera\n' +
            'Enable access for desktop apps'
          : 'To record with camera, please grant camera permission in System Settings.\n\n' +
            'Go to: System Settings > Privacy & Security > Camera\n' +
            'Enable access for Poratake',
        buttons: ['Open Settings', 'Cancel'],
        defaultId: 0,
      };

      const win = BrowserWindow.getFocusedWindow();
      const result = win
        ? await dialog.showMessageBox(win, options)
        : await dialog.showMessageBox(options);

      if (result.response === 0) {
        openCameraPreferences();
      }
      return false;
    }
  }

  return true;
}

export async function checkAndRequestMicrophonePermission(): Promise<boolean> {
  const micStatus = getMicrophoneStatus();

  if (
    micStatus === 'not-determined' ||
    micStatus === 'denied' ||
    micStatus === 'restricted'
  ) {
    const granted = await requestMicrophonePermission();
    if (!granted) {
      const options = {
        type: 'error' as const,
        title: 'Microphone Permission Required',
        message: 'Microphone access is not granted.',
        detail: isWindows
          ? 'To record with microphone, please allow microphone access in Windows Settings.\n\n' +
            'Go to: Settings > Privacy & security > Microphone\n' +
            'Enable access for desktop apps'
          : 'To record with microphone, please grant microphone permission in System Settings.\n\n' +
            'Go to: System Settings > Privacy & Security > Microphone\n' +
            'Enable access for Poratake',
        buttons: ['Open Settings', 'Cancel'],
        defaultId: 0,
      };

      const win = BrowserWindow.getFocusedWindow();
      const result = win
        ? await dialog.showMessageBox(win, options)
        : await dialog.showMessageBox(options);

      if (result.response === 0) {
        openMicrophonePreferences();
      }
      return false;
    }
  }

  return true;
}
