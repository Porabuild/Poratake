import {
  systemPreferences,
  shell,
  ipcMain,
  dialog,
  desktopCapturer,
  BrowserWindow,
} from 'electron';
import type {
  PermissionsState,
  ScreenRecordingStatus,
  MicrophoneStatus,
  CameraStatus,
} from '@/types/permissions';
import {
  hideAreaSelector,
  showAreaSelector,
} from '@/main/capture/area-selector';
import { cleanupRecordingUIForMicPermission } from '@/main/capture/video/cleanup';

export function getScreenRecordingStatus(): ScreenRecordingStatus {
  return systemPreferences.getMediaAccessStatus('screen');
}

export function getMicrophoneStatus(): MicrophoneStatus {
  return systemPreferences.getMediaAccessStatus('microphone');
}

export function getCameraStatus(): CameraStatus {
  return systemPreferences.getMediaAccessStatus('camera');
}

export async function requestMicrophonePermission(): Promise<boolean> {
  const status = getMicrophoneStatus();

  if (status === 'granted') {
    return true;
  }

  if (status === 'denied' || status === 'restricted') {
    return false;
  }

  hideAreaSelector();

  try {
    const result = await systemPreferences.askForMediaAccess('microphone');
    return result;
  } finally {
    showAreaSelector();
  }
}

export async function requestCameraPermission(): Promise<boolean> {
  const status = getCameraStatus();

  if (status === 'granted') {
    return true;
  }

  if (status === 'denied' || status === 'restricted') {
    return false;
  }

  hideAreaSelector();

  try {
    const result = await systemPreferences.askForMediaAccess('camera');
    return result;
  } finally {
    showAreaSelector();
  }
}

export function checkAccessibility(prompt = false): boolean {
  return systemPreferences.isTrustedAccessibilityClient(prompt);
}

export function getPermissionsStatus(): PermissionsState {
  return {
    screenRecording: getScreenRecordingStatus(),
    accessibility: checkAccessibility(false),
    microphone: getMicrophoneStatus(),
    camera: getCameraStatus(),
  };
}

export function areAllPermissionsGranted(): boolean {
  const status = getPermissionsStatus();
  return status.screenRecording === 'granted' && status.accessibility;
}

export async function requestScreenRecordingPermission(): Promise<void> {
  const status = getScreenRecordingStatus();

  if (status !== 'granted') {
    try {
      await desktopCapturer.getSources({
        types: ['screen'],
        thumbnailSize: { width: 1, height: 1 },
      });
    } catch {
      console.warn('Screen recording permission request failed');
    }
  }
}

export async function openScreenRecordingPreferences(): Promise<void> {
  await requestScreenRecordingPermission();

  shell.openExternal(
    'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture'
  );
}

export function openAccessibilityPreferences(): void {
  shell.openExternal(
    'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'
  );
}

export function openMicrophonePreferences(): void {
  shell.openExternal(
    'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone'
  );
}

export function openCameraPreferences(): void {
  shell.openExternal(
    'x-apple.systempreferences:com.apple.preference.security?Privacy_Camera'
  );
}

export async function showMicrophonePermissionDialog(): Promise<boolean> {
  await cleanupRecordingUIForMicPermission();

  const options = {
    type: 'error' as const,
    title: 'Microphone Permission Required',
    message: 'Microphone access is not granted.',
    detail:
      'To use the microphone, please grant microphone permission in System Settings.\n\n' +
      'Go to: System Settings > Privacy & Security > Microphone\n' +
      'Enable access for Capty',
    buttons: ['Open Settings', 'Cancel'],
    defaultId: 0,
  };

  const win = BrowserWindow.getFocusedWindow();
  const result = win
    ? await dialog.showMessageBox(win, options)
    : await dialog.showMessageBox(options);

  if (result.response === 0) {
    openMicrophonePreferences();
    return true;
  }
  return false;
}

export async function showCameraPermissionDialog(): Promise<boolean> {
  await cleanupRecordingUIForMicPermission();

  const options = {
    type: 'error' as const,
    title: 'Camera Permission Required',
    message: 'Camera access is not granted.',
    detail:
      'To use the camera, please grant camera permission in System Settings.\n\n' +
      'Go to: System Settings > Privacy & Security > Camera\n' +
      'Enable access for Capty',
    buttons: ['Open Settings', 'Cancel'],
    defaultId: 0,
  };

  const win = BrowserWindow.getFocusedWindow();
  const result = win
    ? await dialog.showMessageBox(win, options)
    : await dialog.showMessageBox(options);

  if (result.response === 0) {
    openCameraPreferences();
    return true;
  }
  return false;
}

export function initPermissionsIPC(): void {
  ipcMain.handle('permissions:getStatus', () => {
    return getPermissionsStatus();
  });

  ipcMain.on('permissions:openScreenRecording', async () => {
    await openScreenRecordingPreferences();
  });

  ipcMain.on('permissions:openAccessibility', () => {
    openAccessibilityPreferences();
  });

  ipcMain.on('permissions:openMicrophone', () => {
    openMicrophonePreferences();
  });

  ipcMain.handle('permissions:requestAccessibility', () => {
    return checkAccessibility(true);
  });

  ipcMain.handle('permissions:checkAccessibility', () => {
    return checkAccessibility(false);
  });

  ipcMain.handle('permissions:getMicrophoneStatus', () => {
    return getMicrophoneStatus();
  });

  ipcMain.handle('permissions:requestMicrophone', async () => {
    return await requestMicrophonePermission();
  });

  ipcMain.handle('permissions:showMicrophonePermissionDialog', async () => {
    return await showMicrophonePermissionDialog();
  });

  ipcMain.handle('permissions:getCameraStatus', () => {
    return getCameraStatus();
  });

  ipcMain.handle('permissions:requestCamera', async () => {
    return await requestCameraPermission();
  });

  ipcMain.handle('permissions:showCameraPermissionDialog', async () => {
    return await showCameraPermissionDialog();
  });

  ipcMain.on('permissions:openCamera', () => {
    openCameraPreferences();
  });

  ipcMain.handle('permissions:requestAccessibilityForDesktopIcons', () => {
    return checkAccessibility(true);
  });
}
