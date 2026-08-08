import {
  BrowserWindow,
  screen,
  ipcMain,
  clipboard,
  nativeImage,
  app,
} from 'electron';
import path from 'path';
import fs from 'fs';
import { isDev, devServerUrl } from '@/main/utils/env';
import { getThumbnail } from '@/main/utils/thumbnails';
import { deleteHistoryItem, getHistoryItemByPath } from '@/main/history';
import { openScreenshotEditor } from '@/main/capture/screenshot/open-editor';
import { createVideoEditorWindow } from '@/main/capture/video/video-editor';
import { deleteVideo } from '@/main/capture/video/delete-video';
import * as settings from '@/main/settings';
import { registerPreviewExportIpc } from './video-export';
import { animateWindowMove } from '@/main/utils/window-animation';
import type { ContentType, PreviewDisplayInfo } from '@/types/capture-preview';
import type { PreviewCorner } from '@/types/settings';

interface PreviewWindowData {
  window: BrowserWindow;
  filePath: string;
  contentType: ContentType;
  historyId?: string;
  reveal: () => void;
  setAutoDismissPaused: (paused: boolean) => void;
}

const previewWindows: PreviewWindowData[] = [];

const MAX_PREVIEW_WINDOWS = 4;
const PREVIEW_WIDTH = 200;
const PREVIEW_HEIGHT = 140;
const MARGIN_X = 24;
const MARGIN_Y = 24;
const WINDOW_GAP = 12;

const CORNER_ANCHORS: Record<
  PreviewCorner,
  { fromRight: boolean; fromBottom: boolean }
> = {
  'top-left': { fromRight: false, fromBottom: false },
  'top-right': { fromRight: true, fromBottom: false },
  'bottom-left': { fromRight: false, fromBottom: true },
  'bottom-right': { fromRight: true, fromBottom: true },
};
const REVEAL_FALLBACK_MS = 1500;

function getSelectedPreviewDisplay(): Electron.Display {
  const displays = screen.getAllDisplays();
  const selectedDisplayId = settings.getConfig().preview.displayId;
  const selectedDisplay = displays.find(
    display => display.id === selectedDisplayId
  );

  return selectedDisplay ?? screen.getPrimaryDisplay();
}

function getDisplayLabel(display: Electron.Display, index: number): string {
  const primaryDisplayId = screen.getPrimaryDisplay().id;
  const suffix = display.id === primaryDisplayId ? ' (Primary)' : '';

  return `Display ${index + 1}${suffix}`;
}

function getPreviewPosition(index: number): { x: number; y: number } {
  const display = getSelectedPreviewDisplay();
  const { x: displayX, y: displayY, width, height } = display.workArea;
  const anchor =
    CORNER_ANCHORS[settings.getConfig().preview.corner] ??
    CORNER_ANCHORS['bottom-right'];
  const stackOffset = index * (PREVIEW_HEIGHT + WINDOW_GAP);

  const x = anchor.fromRight
    ? displayX + width - MARGIN_X - PREVIEW_WIDTH
    : displayX + MARGIN_X;
  const y = anchor.fromBottom
    ? displayY + height - MARGIN_Y - PREVIEW_HEIGHT - stackOffset
    : displayY + MARGIN_Y + stackOffset;

  return { x, y };
}

function getPreviewDisplays(): PreviewDisplayInfo[] {
  const selectedDisplayId = getSelectedPreviewDisplay().id;

  return screen.getAllDisplays().map((display, index) => ({
    id: display.id,
    label: getDisplayLabel(display, index),
    isSelected: display.id === selectedDisplayId,
  }));
}

function movePreviewsToDisplay(displayId: number): PreviewDisplayInfo[] {
  const displays = screen.getAllDisplays();
  const display = displays.find(item => item.id === displayId);

  if (!display) {
    return getPreviewDisplays();
  }

  settings.updateConfig({
    preview: { ...settings.getConfig().preview, displayId },
  });
  repositionAllWindows();

  return getPreviewDisplays();
}

function persistPreviewDisplayForWindow(window: BrowserWindow): void {
  if (window.isDestroyed()) return;

  const display = screen.getDisplayMatching(window.getBounds());

  if (settings.getConfig().preview.displayId === display.id) return;

  settings.updateConfig({
    preview: { ...settings.getConfig().preview, displayId: display.id },
  });
}

function broadcastDisplaysChanged(): void {
  const displays = getPreviewDisplays();

  previewWindows.forEach(data => {
    if (!data.window.isDestroyed()) {
      data.window.webContents.send(
        'capture-preview:displays-changed',
        displays
      );
    }
  });
}

function repositionAllWindows(): void {
  previewWindows.forEach((data, index) => {
    if (!data.window.isDestroyed()) {
      const { x, y } = getPreviewPosition(index);
      animateWindowMove(data.window, { x, y });
    }
  });
}

function removePreviewWindow(webContentsId: number): void {
  const index = previewWindows.findIndex(
    data =>
      data.window.isDestroyed() || data.window.webContents.id === webContentsId
  );

  if (index !== -1) {
    previewWindows.splice(index, 1);
    repositionAllWindows();
  }
}

function cleanupDestroyedWindows(): void {
  for (let i = previewWindows.length - 1; i >= 0; i--) {
    if (previewWindows[i].window.isDestroyed()) {
      previewWindows.splice(i, 1);
    }
  }
}

export async function showCapturePreview(
  filePath: string,
  contentType: ContentType = 'screenshot',
  historyId?: string
): Promise<void> {
  cleanupDestroyedWindows();

  if (previewWindows.length >= MAX_PREVIEW_WINDOWS) {
    const oldest = previewWindows.shift();
    if (oldest && !oldest.window.isDestroyed()) {
      oldest.window.close();
    }
    repositionAllWindows();
  }

  const thumbnailResult = await getThumbnail(filePath, contentType);

  const newIndex = previewWindows.length;
  const { x, y } = getPreviewPosition(newIndex);

  const previewWindow = new BrowserWindow({
    width: PREVIEW_WIDTH,
    height: PREVIEW_HEIGHT,
    x,
    y,
    frame: false,
    transparent: false,
    backgroundColor: '#1e1e1e',
    resizable: false,
    movable: true,
    skipTaskbar: true,
    show: false,
    hasShadow: true,
    roundedCorners: true,
    focusable: false,
    acceptFirstMouse: true,
    alwaysOnTop: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      devTools: isDev,
      webSecurity: false,
    },
  });

  previewWindow.setVisibleOnAllWorkspaces(true, {
    visibleOnFullScreen: true,
  });

  previewWindow.setAlwaysOnTop(true, 'screen-saver');

  const webContentsId = previewWindow.webContents.id;

  let revealed = false;
  let revealTimer: NodeJS.Timeout | null = null;
  let dismissTimer: NodeJS.Timeout | null = null;
  let dismissPaused = false;

  const clearDismissTimer = () => {
    if (dismissTimer) {
      clearTimeout(dismissTimer);
      dismissTimer = null;
    }
  };

  const scheduleAutoDismiss = () => {
    clearDismissTimer();
    if (!revealed || dismissPaused || previewWindow.isDestroyed()) return;

    const { autoDismiss, autoDismissSeconds } = settings.getConfig().preview;
    if (!autoDismiss || autoDismissSeconds <= 0) return;

    dismissTimer = setTimeout(() => {
      dismissTimer = null;
      if (!previewWindow.isDestroyed()) {
        previewWindow.close();
      }
    }, autoDismissSeconds * 1000);
  };

  const setAutoDismissPaused = (paused: boolean) => {
    if (dismissPaused === paused) return;

    dismissPaused = paused;
    if (paused) {
      clearDismissTimer();
      return;
    }
    scheduleAutoDismiss();
  };

  const reveal = () => {
    if (revealTimer) {
      clearTimeout(revealTimer);
      revealTimer = null;
    }
    if (revealed || previewWindow.isDestroyed()) return;

    revealed = true;
    previewWindow.showInactive();
    scheduleAutoDismiss();
  };

  revealTimer = setTimeout(reveal, REVEAL_FALLBACK_MS);

  const previewData: PreviewWindowData = {
    window: previewWindow,
    filePath,
    contentType,
    historyId,
    reveal,
    setAutoDismissPaused,
  };

  previewWindows.push(previewData);

  if (devServerUrl) {
    previewWindow.loadURL(devServerUrl);
  } else {
    previewWindow.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  previewWindow.webContents.on('did-finish-load', () => {
    previewWindow.webContents.send('load', {
      type: 'capture-preview',
      params: {
        filePath,
        contentType,
        thumbnailBase64: thumbnailResult.base64,
        historyId,
      },
    });
  });

  previewWindow.on('moved', () => {
    persistPreviewDisplayForWindow(previewWindow);
  });

  previewWindow.on('closed', () => {
    if (revealTimer) {
      clearTimeout(revealTimer);
      revealTimer = null;
    }
    clearDismissTimer();
    removePreviewWindow(webContentsId);
  });
}

function getPreviewDataByWebContentsId(
  webContentsId: number
): PreviewWindowData | undefined {
  return previewWindows.find(
    data =>
      !data.window.isDestroyed() && data.window.webContents.id === webContentsId
  );
}

export function registerCapturePreviewIpc(): void {
  registerPreviewExportIpc();

  ipcMain.on('capture-preview:reposition', () => {
    repositionAllWindows();
  });

  ipcMain.on('capture-preview:ready', event => {
    getPreviewDataByWebContentsId(event.sender.id)?.reveal();
  });

  ipcMain.on(
    'capture-preview:set-auto-dismiss-paused',
    (event, paused: boolean) => {
      getPreviewDataByWebContentsId(event.sender.id)?.setAutoDismissPaused(
        paused
      );
    }
  );

  ipcMain.on('capture-preview:close', event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (data && !data.window.isDestroyed()) {
      data.window.close();
    }
  });

  ipcMain.on('capture-preview:copy', event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data) return;

    if (data.contentType === 'video') return;

    const imageBuffer = fs.readFileSync(data.filePath);
    const image = nativeImage.createFromBuffer(imageBuffer);
    clipboard.writeImage(image);

    if (!data.window.isDestroyed()) {
      data.window.close();
    }
  });

  ipcMain.on('capture-preview:open-editor', event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data) return;

    const { filePath, contentType, historyId } = data;

    if (!data.window.isDestroyed()) {
      data.window.close();
    }

    if (contentType === 'video') {
      createVideoEditorWindow(filePath);
      return;
    }

    openScreenshotEditor(filePath, historyId);
  });

  ipcMain.on('capture-preview:delete', async event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data) return;

    const { filePath, contentType } = data;

    if (!data.window.isDestroyed()) {
      data.window.close();
    }

    if (contentType === 'video') {
      await deleteVideo(filePath, { showNotification: false });
      return;
    }

    const historyItem = getHistoryItemByPath(filePath);
    if (!historyItem) return;

    await deleteHistoryItem(historyItem.id);
  });

  ipcMain.on('capture-preview:start-drag', (event, filePath: string) => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data) return;

    const icon = nativeImage.createFromPath(filePath);
    event.sender.startDrag({
      file: filePath,
      icon: icon.resize({ width: 100 }),
    });
  });

  ipcMain.handle('capture-preview:get-displays', () => {
    return getPreviewDisplays();
  });

  ipcMain.handle(
    'capture-preview:move-to-display',
    (_event, displayId: number) => {
      return movePreviewsToDisplay(displayId);
    }
  );

  app.whenReady().then(() => {
    const handleDisplaysChanged = () => {
      repositionAllWindows();
      broadcastDisplaysChanged();
    };

    screen.on('display-added', handleDisplaysChanged);
    screen.on('display-removed', handleDisplaysChanged);
    screen.on('display-metrics-changed', handleDisplaysChanged);
  });
}

export function closeAllPreviewWindows(): void {
  [...previewWindows].forEach(data => {
    if (!data.window.isDestroyed()) {
      data.window.close();
    }
  });
  previewWindows.length = 0;
}
