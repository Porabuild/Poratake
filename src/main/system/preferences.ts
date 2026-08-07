import { systemPreferences, ipcMain, BrowserWindow, shell } from 'electron';
import { isMac, isWindows } from '@/main/utils/platform';

export function getAccentColor(): string {
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
    return getAccentColor();
  });

  ipcMain.on('open-external', (_event, url: string) => {
    shell.openExternal(url);
  });

  const notifyAccentColorChange = () => {
    setTimeout(() => {
      const newColor = getAccentColor();
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
