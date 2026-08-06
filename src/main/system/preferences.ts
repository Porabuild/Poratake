import { systemPreferences, ipcMain, BrowserWindow, shell } from 'electron';

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

  if (process.platform === 'darwin') {
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

    systemPreferences.subscribeNotification(
      'AppleColorPreferencesChangedNotification',
      notifyAccentColorChange
    );
    systemPreferences.subscribeNotification(
      'AppleAquaColorVariantChanged',
      notifyAccentColorChange
    );
  }
}
