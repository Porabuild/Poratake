import {
  BrowserWindow,
  screen,
  ipcMain,
  nativeImage,
  app,
  dialog,
} from 'electron';
import fs from 'fs';
import path from 'path';
import { isDev, devServerUrl } from '@/main/utils/env.ts';
import {
  openScreenshotWindow,
  getWindowData,
  getWindowFromWebContentsId,
} from './open-editor.ts';
import { registerDockWindow } from '@/main/utils/dock';
import type { EditorState } from '@/types/history.ts';

interface PinWindowData {
  window: BrowserWindow;
  pinId: string;
  restoreToEditor: boolean;
  restoreData: {
    filePath: string;
    width: number;
    height: number;
    editorState?: EditorState;
  };
}

const pinWindows: Map<string, PinWindowData> = new Map();

interface PinScreenshotData {
  imageBase64: string;
  editorState: EditorState;
  filePath: string;
  originalWidth: number;
  originalHeight: number;
}

function createPinWindow(
  imageBase64: string,
  width: number,
  height: number,
  restoreData: PinWindowData['restoreData'],
  restoreToEditor: boolean = true
): void {
  const pinId = `pin-${Date.now()}`;

  const primaryDisplay = screen.getPrimaryDisplay();
  const { width: screenWidth } = primaryDisplay.workAreaSize;

  const existingPinCount = pinWindows.size;
  const offsetX = existingPinCount * 30;
  const offsetY = existingPinCount * 30;

  const pinWindow = new BrowserWindow({
    width: width,
    height: height,
    minWidth: 100,
    minHeight: 100,
    x: Math.min(screenWidth - width - 20 - offsetX, screenWidth - 120),
    y: 20 + offsetY,
    alwaysOnTop: true,
    frame: false,
    titleBarStyle: 'customButtonsOnHover',
    resizable: true,
    movable: true,
    minimizable: false,
    maximizable: false,
    closable: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      devTools: isDev,
    },
    show: false,
    transparent: false,
    hasShadow: true,
  });

  if (devServerUrl) {
    pinWindow.loadURL(devServerUrl);
  } else {
    pinWindow.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  pinWindow.webContents.on('did-finish-load', () => {
    pinWindow.webContents.send('load', {
      type: 'pin',
      params: {
        imageBase64,
        width,
        height,
        pinId,
      },
    });
  });

  pinWindow.once('ready-to-show', async () => {
    await registerDockWindow(pinWindow, 'pin');
    app.focus({ steal: true });
    pinWindow.show();
    pinWindow.focus();
  });

  pinWindows.set(pinId, {
    window: pinWindow,
    pinId,
    restoreToEditor,
    restoreData,
  });

  pinWindow.on('closed', () => {
    const pinData = pinWindows.get(pinId);
    if (pinData) {
      if (pinData.restoreToEditor) {
        openScreenshotWindow({
          filePath: pinData.restoreData.filePath,
          width: pinData.restoreData.width,
          height: pinData.restoreData.height,
          editorState: pinData.restoreData.editorState,
        });
      }
      pinWindows.delete(pinId);
    }
  });
}

export function registerIpcHandlers(): void {
  ipcMain.on('screenshot:pin', (event, data: PinScreenshotData) => {
    const {
      imageBase64,
      editorState,
      filePath,
      originalWidth,
      originalHeight,
    } = data;

    const restoreData = {
      filePath,
      width: originalWidth || 800,
      height: originalHeight || 600,
      editorState,
    };

    const windowData = getWindowData(event.sender.id);
    if (windowData && !windowData.window.isDestroyed()) {
      windowData.isClosingConfirmed = true;
      windowData.window.close();
    }

    const image = nativeImage.createFromBuffer(
      Buffer.from(imageBase64, 'base64')
    );
    const { width: pngWidth, height: pngHeight } = image.getSize();

    const primaryDisplay = screen.getPrimaryDisplay();
    const scaleFactor = primaryDisplay.scaleFactor;
    const displayWidth = Math.floor(pngWidth / scaleFactor);
    const displayHeight = Math.floor(pngHeight / scaleFactor);

    createPinWindow(imageBase64, displayWidth, displayHeight, restoreData);
  });

  ipcMain.on('toggle-pin', (event, pinState: boolean) => {
    const win = getWindowFromWebContentsId(event.sender.id);
    if (win && !win.isDestroyed()) {
      win.setAlwaysOnTop(pinState);
    }
  });
}

export async function openImageToPin(): Promise<void> {
  app.focus({ steal: true });

  const result = await dialog.showOpenDialog({
    title: 'Select Image to Pin',
    filters: [
      {
        name: 'Images',
        extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'tiff'],
      },
    ],
    properties: ['openFile'],
  });

  if (result.canceled || result.filePaths.length === 0) {
    return;
  }

  const filePath = result.filePaths[0];

  if (!fs.existsSync(filePath)) {
    return;
  }

  const imageBuffer = fs.readFileSync(filePath);
  const image = nativeImage.createFromBuffer(imageBuffer);

  if (image.isEmpty()) {
    return;
  }

  const imageBase64 = imageBuffer.toString('base64');
  const { width: pngWidth, height: pngHeight } = image.getSize();

  const primaryDisplay = screen.getPrimaryDisplay();
  const scaleFactor = primaryDisplay.scaleFactor;
  const displayWidth = Math.floor(pngWidth / scaleFactor);
  const displayHeight = Math.floor(pngHeight / scaleFactor);

  const restoreData = {
    filePath,
    width: displayWidth,
    height: displayHeight,
  };

  createPinWindow(imageBase64, displayWidth, displayHeight, restoreData, false);
}
