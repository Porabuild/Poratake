import { systemPreferences, ipcMain, BrowserWindow } from 'electron';
import { isMac, isWindows } from '@/main/utils/platform';
import { openExternalUrl } from '@/main/utils/external-url';

export function getSystemAccentColor(): string {
  try {
    const color = systemPreferences.getAccentColor();
    return `#${color.substring(0, 6)}`;
  } catch (error) {
    console.error('Failed to get accent color:', error);
    return '#007AFF';
  }
}

export function init() {
  ipcMain.handle('system:preferences:get-accent-color', () => {
    return getSystemAccentColor();
  });

  ipcMain.on('open-external', (_event, url: unknown) => {
    openExternalUrl(url);
  });

  const notifyAccentColorChange = () => {
    setTimeout(() => {
      const newColor = getSystemAccentColor();
      BrowserWindow.getAllWindows().forEach(window => {
        window.webContents.send(
          'system:preferences:accent-color-changed',
          newColor
        );
      });
    }, 50);
  };

  if (isMac) {
    systemPreferences.subscribeNotification(
      'AppleColorPreferencesChangedNotification',
      notifyAccentColorChange
    );
    systemPreferences.subscribeNotification(
      'AppleAquaColorVariantChanged',
      notifyAccentColorChange
    );
  }

  if (isWindows) {
    systemPreferences.on('accent-color-changed', notifyAccentColorChange);
  }
}
