import {
  BrowserWindow,
  screen,
  ipcMain,
  clipboard,
  nativeImage,
  app,
  shell,
} from 'electron';
import path from 'path';
import fs from 'fs';
import { pathToFileURL } from 'url';
import { appWebPreferences, loadAppWindow } from '@/main/utils/window-factory';
import type { WindowLoadPayload } from '@/types/window-load';
import { getThumbnail } from '@/main/utils/thumbnails';
import {
  deleteHistoryItem,
  getHistoryItemByPath,
  setHistoryFileReleaseHandler,
} from '@/main/history';
import { openScreenshotEditor } from '@/main/capture/screenshot/open-editor';
import { createVideoEditorWindow } from '@/main/capture/video/video-editor';
import { deleteVideo } from '@/main/capture/video/delete-video';
import { getRecordingVideoPath } from '@/main/capture/video/recording-project.ts';
import * as settings from '@/main/settings';
import { registerPreviewExportIpc } from './video-export';
import { animateWindowMove } from '@/main/utils/window-animation';
import { isWindows } from '@/main/utils/platform';
import type { ContentType, PreviewDisplayInfo } from '@/types/capture-preview';
import type { PreviewCorner } from '@/types/settings';

interface PreviewWindowData {
  window: BrowserWindow;
  filePath: string;
  contentType: ContentType;
  historyId?: string;
  historyIdPromise?: Promise<string | undefined>;
  actionReadyPromise?: Promise<unknown>;
  thumbnailReadyPromise?: Promise<void>;
  isDeleting: boolean;
  setAutoDismissPaused: (paused: boolean) => void;
}

export interface CapturePreviewHandle {
  revealed: Promise<void>;
}

export interface CapturePreviewPreparation {
  window: BrowserWindow;
  loaded: Promise<boolean>;
  claimed: boolean;
  dispose: () => void;
}

const previewWindows: PreviewWindowData[] = [];

const MAX_PREVIEW_WINDOWS = 4;
const PREVIEW_WIDTH = 200;
const PREVIEW_HEIGHT = 140;
const MARGIN_X = 24;
const MARGIN_Y = 24;
const WINDOW_GAP = 12;

const IMAGE_MIME_TYPES: Record<string, string> = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
};

function getImageMimeType(filePath: string): string {
  return IMAGE_MIME_TYPES[path.extname(filePath).toLowerCase()] ?? 'image/png';
}

const CORNER_ANCHORS: Record<
  PreviewCorner,
  { fromRight: boolean; fromBottom: boolean }
> = {
  'top-left': { fromRight: false, fromBottom: false },
  'top-right': { fromRight: true, fromBottom: false },
  'bottom-left': { fromRight: false, fromBottom: true },
  'bottom-right': { fromRight: true, fromBottom: true },
};
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

function createPreviewBrowserWindow(index: number): {
  window: BrowserWindow;
  loaded: Promise<boolean>;
} {
  const { x, y } = getPreviewPosition(index);
  const window = new BrowserWindow({
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
    focusable: isWindows,
    acceptFirstMouse: true,
    alwaysOnTop: true,
    webPreferences: appWebPreferences({ backgroundThrottling: false }),
  });

  window.setVisibleOnAllWorkspaces(true, {
    visibleOnFullScreen: true,
  });
  window.setAlwaysOnTop(true, 'screen-saver');

  const loaded = new Promise<boolean>(resolve => {
    const fail = () => {
      resolve(false);
      if (!window.isDestroyed()) {
        window.close();
      }
    };

    window.webContents.ipc.once('capture-preview:renderer-failed', fail);
    window.webContents.once(
      'did-fail-load',
      (_event, _errorCode, _errorDescription, _validatedUrl, isMainFrame) => {
        if (isMainFrame) {
          fail();
        }
      }
    );
    window.webContents.once('render-process-gone', fail);
    window.webContents.ipc.once('capture-preview:renderer-mounted', () => {
      resolve(true);
      if (!window.isDestroyed()) {
        window.webContents.send('capture-preview:prepare-renderer');
      }
    });
  });

  loadAppWindow(window, { type: 'capture-preview' });

  return { window, loaded };
}

function createCapturePreviewPreparation(): CapturePreviewPreparation {
  const prepared = createPreviewBrowserWindow(previewWindows.length);
  const preparation: CapturePreviewPreparation = {
    ...prepared,
    claimed: false,
    dispose: () => {
      if (preparation.claimed) return;

      if (!preparation.window.isDestroyed()) {
        preparation.window.close();
      }
    },
  };

  return preparation;
}

export function prepareCapturePreview(): CapturePreviewPreparation {
  cleanupDestroyedWindows();
  return createCapturePreviewPreparation();
}

export function showCapturePreview(
  filePath: string,
  contentType: ContentType = 'screenshot',
  historyId?: string,
  preparation?: CapturePreviewPreparation,
  historyIdPromise?: Promise<string | undefined>,
  actionReadyPromise?: Promise<unknown>
): CapturePreviewHandle {
  cleanupDestroyedWindows();

  if (previewWindows.length >= MAX_PREVIEW_WINDOWS) {
    const oldest = previewWindows.shift();
    if (oldest && !oldest.window.isDestroyed()) {
      oldest.window.close();
    }
    repositionAllWindows();
  }

  const newIndex = previewWindows.length;
  const { x, y } = getPreviewPosition(newIndex);
  const preparedWindow =
    preparation && !preparation.window.isDestroyed() ? preparation : null;
  const preview =
    preparedWindow ?? createPreviewBrowserWindow(previewWindows.length);
  const previewWindow = preview.window;
  if (preparedWindow) {
    previewWindow.setPosition(x, y);
  }

  const webContentsId = previewWindow.webContents.id;

  let isRevealed = false;
  let dismissTimer: NodeJS.Timeout | null = null;
  let dismissPaused = false;
  let resolveRevealed: () => void = () => {};
  const revealed = new Promise<void>(resolve => {
    resolveRevealed = resolve;
  });

  const clearDismissTimer = () => {
    if (dismissTimer) {
      clearTimeout(dismissTimer);
      dismissTimer = null;
    }
  };

  const scheduleAutoDismiss = () => {
    clearDismissTimer();
    if (!isRevealed || dismissPaused || previewWindow.isDestroyed()) return;

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
    if (isRevealed || previewWindow.isDestroyed()) return;

    isRevealed = true;
    previewWindow.showInactive();
    scheduleAutoDismiss();
    resolveRevealed();
  };

  const previewData: PreviewWindowData = {
    window: previewWindow,
    filePath,
    contentType,
    historyId,
    historyIdPromise,
    actionReadyPromise,
    isDeleting: false,
    setAutoDismissPaused,
  };

  previewWindows.push(previewData);

  previewWindow.webContents.ipc.once('capture-preview:content-ready', reveal);

  const initialImageUrl = pathToFileURL(
    contentType === 'video' ? getRecordingVideoPath(filePath) : filePath
  ).href;

  const sendPreviewData = (thumbnailUrl?: string) => {
    if (previewWindow.isDestroyed()) return;

    const payload: WindowLoadPayload = {
      type: 'capture-preview',
      params: {
        filePath,
        contentType,
        imageUrl: initialImageUrl,
        thumbnailUrl,
        historyId,
      },
    };
    previewWindow.webContents.send('load', payload);
  };

  void preview.loaded.then(loaded => {
    if (!loaded) return;

    sendPreviewData();
    if (previewWindow.isDestroyed()) return;

    previewData.thumbnailReadyPromise = getThumbnail(filePath, contentType)
      .then(result => {
        if (!result?.base64) return;
        sendPreviewData(`data:image/jpeg;base64,${result.base64}`);
      })
      .catch(() => {});
  });

  previewWindow.on('moved', () => {
    persistPreviewDisplayForWindow(previewWindow);
  });

  previewWindow.on('closed', () => {
    resolveRevealed();
    clearDismissTimer();
    removePreviewWindow(webContentsId);
  });

  if (preparedWindow) {
    preparedWindow.claimed = true;
  }
  return { revealed };
}

function getPreviewDataByWebContentsId(
  webContentsId: number
): PreviewWindowData | undefined {
  return previewWindows.find(
    data =>
      !data.window.isDestroyed() && data.window.webContents.id === webContentsId
  );
}

export function getCapturePreviewUploadPath(
  webContentsId: number
): string | null {
  return getPreviewDataByWebContentsId(webContentsId)?.filePath ?? null;
}

async function releaseCapturePreviewFile(filePath: string): Promise<void> {
  const targetPath = path.resolve(filePath);
  const matchingPreviews = previewWindows.filter(data => {
    const previewPath = path.resolve(data.filePath);
    return isWindows
      ? previewPath.toLowerCase() === targetPath.toLowerCase()
      : previewPath === targetPath;
  });

  await Promise.all(
    matchingPreviews.map(async data => {
      data.isDeleting = true;
      const previewClosed = data.window.isDestroyed()
        ? Promise.resolve()
        : new Promise<void>(resolve => {
            data.window.once('closed', resolve);
            data.window.close();
          });
      await Promise.allSettled([
        data.actionReadyPromise,
        data.thumbnailReadyPromise,
        previewClosed,
      ]);
    })
  );
}

export function registerCapturePreviewIpc(): void {
  setHistoryFileReleaseHandler(releaseCapturePreviewFile);
  registerPreviewExportIpc(webContentsId => {
    const data = getPreviewDataByWebContentsId(webContentsId);
    return data?.contentType === 'video' ? data.filePath : null;
  });

  ipcMain.on('capture-preview:reposition', () => {
    repositionAllWindows();
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

  ipcMain.on('capture-preview:show-in-folder', event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data) return;

    shell.showItemInFolder(data.filePath);
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

  ipcMain.handle('capture-preview:get-source-image', event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data || data.contentType === 'video') return null;

    try {
      const buffer = fs.readFileSync(data.filePath);
      return `data:${getImageMimeType(data.filePath)};base64,${buffer.toString('base64')}`;
    } catch (error) {
      console.error('Failed to read preview source image:', error);
      return null;
    }
  });

  ipcMain.handle(
    'capture-preview:copy-composited',
    (event, dataUrl: string) => {
      const data = getPreviewDataByWebContentsId(event.sender.id);
      if (!data) return false;

      const image = nativeImage.createFromDataURL(dataUrl);
      if (image.isEmpty()) return false;

      clipboard.writeImage(image);

      if (!data.window.isDestroyed()) {
        data.window.close();
      }
      return true;
    }
  );

  ipcMain.on('capture-preview:open-editor', async event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data) return;

    const {
      filePath,
      contentType,
      historyId,
      historyIdPromise,
      actionReadyPromise,
    } = data;

    if (!data.window.isDestroyed()) {
      data.window.close();
    }

    if (contentType === 'video') {
      await actionReadyPromise;
      createVideoEditorWindow(filePath);
      return;
    }

    const resolvedHistoryId =
      historyId ??
      (await historyIdPromise) ??
      getHistoryItemByPath(filePath)?.id;
    openScreenshotEditor(filePath, resolvedHistoryId);
  });

  ipcMain.on('capture-preview:delete', async event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data || data.isDeleting) return;
    data.isDeleting = true;

    const {
      filePath,
      contentType,
      historyId,
      historyIdPromise,
      actionReadyPromise,
    } = data;

    const previewClosed = new Promise<void>(resolve => {
      data.window.once('closed', resolve);
      data.window.close();
    });

    if (contentType === 'video') {
      await Promise.allSettled([
        historyIdPromise,
        actionReadyPromise,
        data.thumbnailReadyPromise,
        previewClosed,
      ]);
      await deleteVideo(filePath, { showNotification: false });
      return;
    }

    await previewClosed;

    const resolvedHistoryId =
      historyId ??
      (await historyIdPromise) ??
      getHistoryItemByPath(filePath)?.id;
    if (!resolvedHistoryId) return;

    await deleteHistoryItem(resolvedHistoryId);
  });

  ipcMain.on('capture-preview:start-drag', event => {
    const data = getPreviewDataByWebContentsId(event.sender.id);
    if (!data) return;

    const { filePath } = data;
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
