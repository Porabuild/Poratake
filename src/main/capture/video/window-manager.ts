import { BrowserWindow, screen, app, dialog } from 'electron';
import fs from 'fs';
import {
  appWebPreferences,
  centeredPosition,
  loadAppWindow,
} from '@/main/utils/window-factory';
import { isRecordingProject, getRecordingVideoPath } from './recording-project';
import { registerDockWindow } from '@/main/utils/dock';
import { sendWindowLoad } from '@/main/utils/window-load';
import {
  titleBarWindowOptions,
  trackTitleBarTheme,
} from '@/main/utils/title-bar';
import { probeVideo, type VideoProbeResult } from '@/main/utils/ffmpeg';
import { MusicRetentionSession } from './music-retention';

export interface VideoEditorWindowData {
  window: BrowserWindow;
  filePath: string;
  isClosingConfirmed: boolean;
  closeHandlerReady?: boolean;
  closeSavePending?: boolean;
  isExporting: boolean;
  musicSession?: MusicRetentionSession | null;
  retentionAbandoned?: boolean;
  exportCloseWarningShown?: boolean;
  videoProbe?: Promise<VideoProbeResult | null>;
}

const videoEditorWindows = new Map<number, VideoEditorWindowData>();

export function getWindowData(
  webContentsId: number
): VideoEditorWindowData | undefined {
  return videoEditorWindows.get(webContentsId);
}

export function setWindowData(
  webContentsId: number,
  data: VideoEditorWindowData
): void {
  videoEditorWindows.set(webContentsId, data);
}

export function deleteWindowData(webContentsId: number): void {
  videoEditorWindows.delete(webContentsId);
}

export function updateWindowFilePath(
  webContentsId: number,
  newFilePath: string
): void {
  const data = videoEditorWindows.get(webContentsId);
  if (data) {
    data.filePath = newFilePath;
    data.videoProbe = probeVideo(newFilePath);
    data.musicSession?.updateProject(newFilePath);
  }
}

export function setWindowExporting(
  webContentsId: number,
  isExporting: boolean
): void {
  const data = videoEditorWindows.get(webContentsId);
  if (!data) return;
  data.isExporting = isExporting;
  data.exportCloseWarningShown = false;
}

export function getWindowFromWebContentsId(
  webContentsId: number
): BrowserWindow | null {
  const data = videoEditorWindows.get(webContentsId);
  return data?.window ?? null;
}

export function getVideoEditorWindowsCount(): number {
  return videoEditorWindows.size;
}

export function createVideoEditorWindow(
  inputPath: string
): BrowserWindow | undefined {
  const videoPath = isRecordingProject(inputPath)
    ? getRecordingVideoPath(inputPath)
    : inputPath;

  if (!fs.existsSync(videoPath)) {
    console.error('Video file not found:', videoPath);
    return;
  }

  let musicSession: MusicRetentionSession | null;
  try {
    musicSession = MusicRetentionSession.open(videoPath);
  } catch (error) {
    dialog.showErrorBox(
      'Could not open recording',
      `The recording's music files are in use: ${error instanceof Error ? error.message : 'unknown error'}`
    );
    return;
  }

  const primaryDisplay = screen.getPrimaryDisplay();
  const { width: screenWidth, height: screenHeight } =
    primaryDisplay.workAreaSize;

  const windowWidth = Math.min(1280, screenWidth - 100);
  const windowHeight = Math.min(800, screenHeight - 100);

  const existingWindowCount = videoEditorWindows.size;
  const positionOffset = existingWindowCount * 30;

  let newWindow: BrowserWindow;
  try {
    newWindow = new BrowserWindow({
      width: windowWidth,
      height: windowHeight,
      minWidth: 1200,
      minHeight: 750,
      maximizable: true,
      minimizable: true,
      resizable: true,
      webPreferences: appWebPreferences(),
      alwaysOnTop: false,
      ...titleBarWindowOptions(),
      ...centeredPosition(
        { width: windowWidth, height: windowHeight },
        positionOffset
      ),
      show: false,
      backgroundColor: '#1e1e1e',
    });
  } catch (error) {
    musicSession?.close();
    throw error;
  }

  trackTitleBarTheme(newWindow);

  const webContentsId = newWindow.webContents.id;

  videoEditorWindows.set(webContentsId, {
    window: newWindow,
    filePath: videoPath,
    isClosingConfirmed: false,
    isExporting: false,
    musicSession,
    videoProbe: probeVideo(videoPath),
  });

  loadAppWindow(newWindow, { type: 'video-editor' });

  newWindow.webContents.on('did-finish-load', () => {
    const currentData = videoEditorWindows.get(webContentsId);
    if (!currentData) return;
    sendWindowLoad(newWindow.webContents, {
      type: 'video-editor',
      params: {
        filePath: currentData.filePath,
      },
    });
  });

  newWindow.webContents.on('render-process-gone', () => {
    const data = videoEditorWindows.get(webContentsId);
    if (!data) return;
    data.retentionAbandoned = true;
    data.closeHandlerReady = false;
    if (!data.closeSavePending || newWindow.isDestroyed()) return;
    data.closeSavePending = false;
    data.isClosingConfirmed = true;
    newWindow.close();
  });

  newWindow.once('ready-to-show', async () => {
    await registerDockWindow(newWindow, 'video-editor');
    app.focus({ steal: true });
    newWindow.show();
    newWindow.focus();
  });

  newWindow.on('close', event => {
    const windowData = videoEditorWindows.get(webContentsId);
    if (!windowData) return;

    if (windowData.isExporting && !windowData.isClosingConfirmed) {
      event.preventDefault();
      if (!windowData.exportCloseWarningShown) {
        windowData.exportCloseWarningShown = true;
        void dialog
          .showMessageBox(newWindow, {
            type: 'info',
            message: 'Wait for the video export to finish before closing.',
            buttons: ['OK'],
          })
          .catch(error =>
            console.error('Failed to show export message:', error)
          );
      }
      return;
    }

    if (windowData.isClosingConfirmed || !windowData.closeHandlerReady) return;
    event.preventDefault();
    if (windowData.closeSavePending) return;
    windowData.closeSavePending = true;
    newWindow.webContents.send('video-editor:prepare-close');
  });

  newWindow.on('closed', () => {
    const data = videoEditorWindows.get(webContentsId);
    if (data && !data.retentionAbandoned) data.musicSession?.close();
    videoEditorWindows.delete(webContentsId);
  });

  return newWindow;
}

export function getVideoEditorWindow(
  webContentsId: number
): BrowserWindow | null {
  return getWindowFromWebContentsId(webContentsId);
}

export async function openVideoInEditor(): Promise<void> {
  app.focus({ steal: true });

  const result = await dialog.showOpenDialog({
    title: 'Select Video to Edit',
    filters: [
      {
        name: 'Videos',
        extensions: ['mov', 'mp4', 'webm', 'm4v', 'avi', 'mkv'],
      },
    ],
    properties: ['openFile'],
  });

  if (!result.canceled && result.filePaths.length > 0) {
    createVideoEditorWindow(result.filePaths[0]);
  }
}
