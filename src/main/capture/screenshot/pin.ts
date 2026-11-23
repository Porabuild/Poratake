import {
  BrowserWindow,
  screen,
  ipcMain,
  nativeImage,
  app,
  dialog,
} from 'electron';
import fs from 'fs';
import { appWebPreferences, loadAppWindow } from '@/main/utils/window-factory';
import {
  openScreenshotWindow,
  getWindowData,
  getWindowFromWebContentsId,
} from './open-editor.ts';
import { registerDockWindow } from '@/main/utils/dock';
import { sendWindowLoad } from '@/main/utils/window-load';
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

const PREVIEW_WIDTH = 200;
const PREVIEW_HEIGHT = 140;
const PREVIEW_MARGIN = 24;
const PIN_MIN_SIZE = 48;
const PIN_OFFSET = 30;

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
  const workArea = primaryDisplay.workArea ?? {
    x: 0,
    y: 0,
    width: primaryDisplay.workAreaSize.width,
    height: primaryDisplay.workAreaSize.height,
  };
  const safeWidth = Math.max(width, 1);
  const safeHeight = Math.max(height, 1);
  const maxWidth = Math.max(Math.min(PREVIEW_WIDTH, workArea.width), 1);
  const maxHeight = Math.max(Math.min(PREVIEW_HEIGHT, workArea.height), 1);
  const minWidth = Math.min(PIN_MIN_SIZE, maxWidth);
  const minHeight = Math.min(PIN_MIN_SIZE, maxHeight);
  const scale = Math.min(1, maxWidth / safeWidth, maxHeight / safeHeight);
  const scaledWidth = Math.floor(safeWidth * scale);
  const scaledHeight = Math.floor(safeHeight * scale);
  const windowWidth = Math.min(Math.max(scaledWidth, minWidth), maxWidth);
  const windowHeight = Math.min(Math.max(scaledHeight, minHeight), maxHeight);

  const existingPinCount = pinWindows.size;
  const offset = existingPinCount * PIN_OFFSET;

  const pinWindow = new BrowserWindow({
    width: windowWidth,
    height: windowHeight,
    minWidth,
    minHeight,
    x:
      workArea.x +
      Math.max(0, workArea.width - windowWidth - PREVIEW_MARGIN - offset),
    y: workArea.y + PREVIEW_MARGIN + offset,
    alwaysOnTop: true,
    frame: false,
    titleBarStyle: 'customButtonsOnHover',
    resizable: true,
    movable: true,
    minimizable: false,
    maximizable: false,
    closable: true,
    webPreferences: appWebPreferences(),
    show: false,
    transparent: false,
    hasShadow: true,
  });

  if (scaledWidth >= minWidth && scaledHeight >= minHeight) {
    pinWindow.setAspectRatio(safeWidth / safeHeight);
  }

  loadAppWindow(pinWindow);

  pinWindow.webContents.on('did-finish-load', () => {
    sendWindowLoad(pinWindow.webContents, {
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
