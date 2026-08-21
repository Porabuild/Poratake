import { BrowserWindow, ipcMain } from 'electron';
import { isSettingsWindowWebContents } from '@/main/settings/window';
import {
  applyTitleBarAppearance,
  supportsNativeWindowMaterial,
  supportsWindowsAcrylic,
} from '@/main/utils/title-bar';
import type { SettingsConfig } from '@/types/settings.ts';
import { getAppVersion } from '@/main/utils/env.ts';
import { getSettingsUiConfig } from './migrations.ts';
import {
  applyLoginItemSetting,
  getConfig,
  resetConfig,
  retainCloudUpdate,
  updateConfig,
} from './store.ts';

export function registerSettingsIpc(): void {
  ipcMain.handle('settings:get-ui', event => {
    return getSettingsUiConfig(
      getConfig(),
      isSettingsWindowWebContents(event.sender)
    );
  });

  ipcMain.handle('settings:get-appearance', () => {
    return getConfig().appearance;
  });

  ipcMain.handle('settings:apply-window-material', event => {
    const nativeCapable = supportsNativeWindowMaterial();
    const window = BrowserWindow.fromWebContents(event.sender);
    if (!nativeCapable || !window || window.isDestroyed()) {
      return { nativeCapable: false };
    }

    if (supportsWindowsAcrylic()) {
      window.setBackgroundMaterial('acrylic');
      window.setBackgroundColor('#00000000');
    }
    return { nativeCapable: true };
  });

  ipcMain.handle(
    'settings:update',
    (event, updates: Partial<SettingsConfig>) => {
      const isSettingsWindow = isSettingsWindowWebContents(event.sender);
      const allowedUpdates = { ...updates };
      Reflect.deleteProperty(allowedUpdates, 'wallpaper');
      if (!isSettingsWindow) {
        Reflect.deleteProperty(allowedUpdates, 'cloud');
      }
      if (
        isSettingsWindow &&
        allowedUpdates.cloud &&
        !retainCloudUpdate(allowedUpdates.cloud)
      ) {
        console.error(
          'Cloud settings update rejected: OS encryption unavailable'
        );
        Reflect.deleteProperty(allowedUpdates, 'cloud');
      }
      const updatedConfig = updateConfig(allowedUpdates);

      if (allowedUpdates.appearance) {
        applyTitleBarAppearance(updatedConfig.appearance);
        BrowserWindow.getAllWindows().forEach(window => {
          window.webContents.send(
            'settings:appearance-updated',
            updatedConfig.appearance
          );
        });
      }

      if (allowedUpdates.screenshot) {
        BrowserWindow.getAllWindows().forEach(window => {
          window.webContents.send('screenshot-settings:updated', {
            closeOnCopy: updatedConfig.screenshot.closeOnCopy,
            closeOnSave: updatedConfig.screenshot.closeOnSave,
            format: updatedConfig.screenshot.format,
          });
        });
      }

      return getSettingsUiConfig(updatedConfig, isSettingsWindow);
    }
  );

  ipcMain.handle('settings:reset', event => {
    if (!isSettingsWindowWebContents(event.sender)) {
      return getSettingsUiConfig(getConfig(), false);
    }

    const config = resetConfig();
    applyTitleBarAppearance(config.appearance);
    applyLoginItemSetting();
    BrowserWindow.getAllWindows().forEach(window => {
      window.webContents.send('settings:appearance-updated', config.appearance);
    });
    return config;
  });

  ipcMain.handle('app:getVersion', () => {
    return getAppVersion();
  });

  ipcMain.handle('editor:getPreferences', () => {
    return getConfig().editor;
  });

  ipcMain.handle(
    'editor:updatePreferences',
    (_event, updates: Partial<SettingsConfig['editor']>) => {
      return updateConfig({
        editor: { ...getConfig().editor, ...updates },
      }).editor;
    }
  );
}
