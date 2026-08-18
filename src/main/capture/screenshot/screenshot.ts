import { randomUUID } from 'crypto';
import { selectDisplay, displayFromSelection } from '../display-selector';
import {
  screen,
  ipcMain,
  app,
  dialog,
  Notification,
  BrowserWindow,
} from 'electron';
import fs from 'fs';
import path from 'path';
import { getConfig } from '@/main/settings';
import { daemon } from '@/main/daemon';
import {
  hideDesktopIcons,
  showDesktopIcons,
  isSupported as isDesktopIconsSupported,
  checkAccessibilityPermission,
} from '@/main/capture/desktop-icons';

import {
  deleteHistoryItem,
  getHistoryItem,
  getHistoryItemByPath,
  isHistoryPopoverWebContents,
  updateHistoryItemByPath,
} from '@/main/history';
import { getWindowData, getWindowFromWebContentsId } from './open-editor.ts';
import {
  generateScreenshotPath,
  generateScreenshotExportName,
} from './utils.ts';
import {
  rememberSaveDirectory,
  resolveSaveDialogPath,
} from '@/main/utils/save-location';
import type { EditorState } from '@/types/history.ts';
import type { ScreenshotFormat } from '@/types/settings';
import { openScreenshotFromHistory } from '@/main/capture/screenshot/open-from-history.ts';
import { createOrShowSettingsWindow } from '@/main/settings';
import {
  finalizeCapture,
  prepareScreenshotPreview,
} from '@/main/capture/screenshot/finalize';
import { isFeatureSupported } from '@/main/system/capabilities';
import { captureDisplayToFile } from '@/main/capture/screenshot/native-capture';
import {
  captureAreaToFile,
  captureWindowToFile,
} from '@/main/capture/area-overlay';
import type { CapturePreviewPreparation } from '@/main/capture/capture-preview';

export type CaptureMode = 'screen' | 'area' | 'window';

async function withHiddenDesktopIcons<T>(
  capture: () => Promise<T>,
  afterCapture: (result: T) => Promise<void>
): Promise<T> {
  const config = getConfig();
  const shouldHideIcons =
    config.screenshot.hideDesktopIcons &&
    isDesktopIconsSupported() &&
    checkAccessibilityPermission(false);

  if (shouldHideIcons) {
    await hideDesktopIcons('capture');
  }

  let result: T;
  try {
    result = await capture();
  } catch (error) {
    if (shouldHideIcons) {
      await showDesktopIcons('capture');
    }
    throw error;
  }

  const restoreIcons = shouldHideIcons
    ? showDesktopIcons('capture')
    : Promise.resolve(true);

  try {
    await afterCapture(result);
  } finally {
    await restoreIcons;
  }

  return result;
}

async function captureScreenWithDisplaySelector(
  preparation?: CapturePreviewPreparation | null
): Promise<void> {
  let display = screen.getDisplayNearestPoint(screen.getCursorScreenPoint());

  if (
    screen.getAllDisplays().length > 1 &&
    isFeatureSupported('display-selector')
  ) {
    try {
      const selection = await selectDisplay();
      if (selection.status === 'cancelled') {
        return;
      }
      display = displayFromSelection(selection) ?? display;
    } catch (error) {
      console.error('Display selection failed:', error);
    }
  }

  const screenshotPath = generateScreenshotPath();
  const captured = await withHiddenDesktopIcons(
    () => captureDisplayToFile(display, screenshotPath),
    result =>
      result ? finalizeCapture(screenshotPath, preparation) : Promise.resolve()
  );

  if (!captured) {
    console.error('Screen capture failed');
    return;
  }
}

async function captureWindowWithSelector(
  preparation?: CapturePreviewPreparation | null
): Promise<void> {
  const screenshotPath = generateScreenshotPath();
  const captured = await captureWindowToFile(screenshotPath);

  if (!captured) {
    console.error('Window capture failed');
    return;
  }

  await finalizeCapture(screenshotPath, preparation);
}

async function captureAreaWithSelector(
  preparation?: CapturePreviewPreparation | null
): Promise<void> {
  const screenshotPath = generateScreenshotPath();
  const captured = await withHiddenDesktopIcons(
    () => captureAreaToFile(screenshotPath),
    result =>
      result ? finalizeCapture(screenshotPath, preparation) : Promise.resolve()
  );

  if (!captured) {
    return;
  }
}

export default async function screenshot(mode: CaptureMode = 'area') {
  const preparation = prepareScreenshotPreview();

  try {
    switch (mode) {
      case 'screen':
        return await captureScreenWithDisplaySelector(preparation);
      case 'window':
        if (!isFeatureSupported('screenshot-window')) {
          console.warn('Window capture is not supported on this platform');
          return;
        }
        return await captureWindowWithSelector(preparation);
      case 'area':
        return await captureAreaWithSelector(preparation);
    }
  } finally {
    preparation?.dispose();
  }
}

export function registerIpcHandlers(): void {
  ipcMain.on('screenshot:close-confirmed', event => {
    const data = getWindowData(event.sender.id);
    if (data && !data.window.isDestroyed()) {
      data.isClosingConfirmed = true;
      data.window.close();
    }
  });

  ipcMain.on('screenshot:copy-from-menu', event => {
    const win = getWindowFromWebContentsId(event.sender.id);
    if (win && !win.isDestroyed()) {
      win.webContents.send('screenshot:copy');
    }
  });

  ipcMain.on('save-screenshot', async event => {
    const data = getWindowData(event.sender.id);
    if (!data || !fs.existsSync(data.filePath)) {
      return;
    }

    const { filePath } = await dialog.showSaveDialog({
      defaultPath: resolveSaveDialogPath(
        'screenshot',
        generateScreenshotExportName('png'),
        app.getPath('pictures')
      ),
      filters: [{ name: 'Images', extensions: ['png'] }],
    });

    if (!filePath) {
      return;
    }

    rememberSaveDirectory('screenshot', filePath);
    fs.copyFileSync(data.filePath, filePath);
    event.sender.send('screenshot:saved');
  });

  ipcMain.on(
    'screenshot:save-edited',
    async (event, imageBase64: string, format: ScreenshotFormat = 'png') => {
      const data = getWindowData(event.sender.id);
      if (!data) {
        return;
      }

      const extension = format === 'jpeg' ? 'jpg' : 'png';
      const filterName = format === 'jpeg' ? 'JPEG Image' : 'PNG Image';

      const { filePath } = await dialog.showSaveDialog({
        defaultPath: resolveSaveDialogPath(
          'screenshot',
          generateScreenshotExportName(extension),
          app.getPath('pictures')
        ),
        filters: [{ name: filterName, extensions: [extension] }],
      });

      if (!filePath) {
        return;
      }

      rememberSaveDirectory('screenshot', filePath);
      fs.writeFileSync(filePath, Buffer.from(imageBase64, 'base64'));
      event.sender.send('screenshot:saved');
    }
  );

  ipcMain.on('get-screenshot-path', event => {
    const data = getWindowData(event.sender.id);
    event.returnValue = data?.filePath ?? null;
  });

  ipcMain.handle('screenshot:read-file', async event => {
    const data = getWindowData(event.sender.id);
    if (!data || !fs.existsSync(data.filePath)) {
      throw new Error('File not found');
    }
    const imageBuffer = await fs.promises.readFile(data.filePath);
    return imageBuffer.toString('base64');
  });

  ipcMain.on(
    'history:save-editor-state',
    async (event, editorState: EditorState) => {
      const data = getWindowData(event.sender.id);
      if (data?.filePath) {
        await updateHistoryItemByPath(data.filePath, editorState);
      }
    }
  );

  ipcMain.on(
    'screenshot:sync-state',
    (event, state: { editorState: EditorState | null }) => {
      const data = getWindowData(event.sender.id);
      if (data) {
        data.editorState = state.editorState;
      }
    }
  );

  ipcMain.on('history:openScreenshot', (event, id: string) => {
    if (!isHistoryPopoverWebContents(event.sender)) return;

    const item = getHistoryItem(id);
    if (!item || item.type !== 'screenshot') return;

    openScreenshotFromHistory(item);
  });

  ipcMain.on('open-settings', (_event, tab?: string) => {
    createOrShowSettingsWindow(tab);
  });

  ipcMain.handle('screenshot:confirmDelete', async event => {
    const parentWindow = BrowserWindow.fromWebContents(event.sender);
    const options = {
      type: 'none' as const,
      title: 'Delete Screenshot?',
      message: 'Delete Screenshot?',
      detail:
        'This will permanently delete the current screenshot. This action cannot be undone.',
      buttons: ['Cancel', 'Delete'],
      defaultId: 1,
      cancelId: 0,
    };

    const result = parentWindow
      ? await dialog.showMessageBox(parentWindow, options)
      : await dialog.showMessageBox(options);

    return result.response === 1;
  });

  ipcMain.on('screenshot:delete', async event => {
    const data = getWindowData(event.sender.id);
    if (!data) return;

    const filePath = data.filePath;

    if (!data.window.isDestroyed()) {
      data.isClosingConfirmed = true;
      data.window.close();
    }

    const historyItem = getHistoryItemByPath(filePath);
    if (historyItem) {
      await deleteHistoryItem(historyItem.id);
      if (getConfig().general.showDeletionNotifications) {
        new Notification({
          title: 'Screenshot Deleted',
          body: 'The screenshot has been permanently deleted.',
        }).show();
      }
    }
  });

  ipcMain.handle('screenshot:print', async (_event, imageBase64: string) => {
    if (!isFeatureSupported('print')) {
      return;
    }
    await daemon.call('print', 'image', { imageBase64 });
  });

  ipcMain.handle('screenshot:capture-for-editor', async event => {
    const win = BrowserWindow.fromWebContents(event.sender);
    if (!win || win.isDestroyed()) return null;

    const temporaryPath = path.join(
      app.getPath('temp'),
      `poratake-editor-${randomUUID()}.png`
    );

    const wasVisible = win.isVisible();
    if (wasVisible) {
      const hidden = new Promise<void>(resolve => {
        win.once('hide', resolve);
      });
      win.hide();
      await hidden;
    }

    try {
      const captured = await captureAreaToFile(temporaryPath);
      if (!captured || !fs.existsSync(temporaryPath)) {
        return null;
      }

      return fs.readFileSync(temporaryPath).toString('base64');
    } catch (error) {
      console.error('Capture for editor failed:', error);
      return null;
    } finally {
      try {
        fs.rmSync(temporaryPath, { force: true });
      } catch (error) {
        console.error('Failed to delete editor capture:', error);
      }
      if (wasVisible && !win.isDestroyed()) {
        win.show();
        win.focus();
      }
    }
  });
}
