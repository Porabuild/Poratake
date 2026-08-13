import { ipcMain, clipboard } from 'electron';
import { existsSync, unlinkSync, readFileSync } from 'fs';
import { app } from 'electron';
import path from 'path';
import { randomUUID } from 'crypto';
import { execFile } from 'child_process';
import { pathToFileURL } from 'url';
import { probeVideo } from '@/main/utils/ffmpeg';
import {
  getRecordingVideoPath,
  getSystemAudioPath,
  getMicAudioPath,
  getEditorStatePath,
  isRecordingProject,
} from '@/main/capture/video/recording-project';
import { loadCursorData } from '@/main/capture/video/cursor-data';
import {
  loadCameraData,
  getAbsoluteCameraVideoPath,
} from '@/main/capture/video/camera-data';
import type { VideoMetadata } from '@/types/video';
import type { CursorData, CursorStyle } from '@/types/cursor';
import {
  mapVideoRangesToCameraSegments,
  type CameraStyle,
  type CameraSegment,
} from '@/types/camera';
import type { AudioStyle } from '@/types/audio';
import type { MusicTrack } from '@/types/music';
import type { VideoEditorState } from '@/types/video-editor-state';
import {
  authorizeExportOutputPaths,
  isExportOutputPathAllowed,
} from '@/main/capture/video/ipc/export-session';

interface PreviewExportData {
  videoPath: string;
  videoMetadata: VideoMetadata;
  segments: VideoEditorState['segments'] | null;
  zoomSegments: VideoEditorState['zoomSegments'] | null;
  zoomSettings: VideoEditorState['zoomSettings'] | null;
  drawingSegments: VideoEditorState['drawingSegments'] | null;
  cursorData: CursorData | null;
  cursorStyle: CursorStyle | null;
  cameraVideoPath: string | null;
  cameraStyle: CameraStyle | null;
  cameraVisibleRanges: CameraSegment[] | null;
  systemAudioPath: string | null;
  micAudioPath: string | null;
  hasEmbeddedAudio: boolean;
  audioStyle: AudioStyle | null;
  musicTracks: MusicTrack[] | null;
}

const CLIPBOARD_CLEANUP_MS = 5 * 60 * 1000;

function writeWindowsFileToClipboard(filePath: string): Promise<void> {
  return new Promise((resolve, reject) => {
    execFile(
      'powershell.exe',
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        'Set-Clipboard -LiteralPath $env:PORATAKE_CLIPBOARD_FILE',
      ],
      {
        env: { ...process.env, PORATAKE_CLIPBOARD_FILE: filePath },
        windowsHide: true,
      },
      error => {
        if (error) {
          reject(error);
          return;
        }
        resolve();
      }
    );
  });
}

function loadEditorState(filePath: string): Partial<VideoEditorState> | null {
  const statePath = getEditorStatePath(filePath);
  if (!statePath || !existsSync(statePath)) return null;

  try {
    const content = readFileSync(statePath, 'utf-8');
    return JSON.parse(content);
  } catch {
    return null;
  }
}

export function registerPreviewExportIpc(
  getPreviewFilePath: (webContentsId: number) => string | null
): void {
  ipcMain.handle(
    'capture-preview:load-export-data',
    async (event): Promise<PreviewExportData | null> => {
      const filePath = getPreviewFilePath(event.sender.id);
      if (!filePath) return null;

      const videoPath = isRecordingProject(filePath)
        ? getRecordingVideoPath(filePath)
        : filePath;

      const probeResult = await probeVideo(videoPath);
      if (!probeResult) return null;

      const cursorData = await loadCursorData(filePath);
      const cameraData = await loadCameraData(filePath);
      const cameraVideoPath = cameraData
        ? getAbsoluteCameraVideoPath(filePath, cameraData)
        : null;

      const systemAudioFilePath = getSystemAudioPath(filePath);
      const micAudioFilePath = getMicAudioPath(filePath);
      const systemAudioExists = existsSync(systemAudioFilePath);
      const micAudioExists = existsSync(micAudioFilePath);

      const hasEmbeddedAudio =
        !systemAudioExists && !micAudioExists && probeResult.hasAudio;

      const editorState = loadEditorState(filePath);

      return {
        videoPath,
        videoMetadata: probeResult.metadata,
        segments: editorState?.segments ?? null,
        zoomSegments: editorState?.zoomSegments ?? null,
        zoomSettings: editorState?.zoomSettings ?? null,
        drawingSegments: editorState?.drawingSegments ?? null,
        cursorData,
        cursorStyle: (editorState?.cursorStyle as CursorStyle) ?? null,
        cameraVideoPath:
          cameraVideoPath && existsSync(cameraVideoPath)
            ? cameraVideoPath
            : null,
        cameraStyle: (editorState?.cameraStyle as CameraStyle) ?? null,
        cameraVisibleRanges:
          editorState?.cameraSegments ??
          (cameraData
            ? mapVideoRangesToCameraSegments(
                cameraData.meta?.visibleRanges ?? null,
                editorState?.segments ?? [],
                probeResult.metadata.duration
              )
            : null),
        systemAudioPath: systemAudioExists ? systemAudioFilePath : null,
        micAudioPath: micAudioExists ? micAudioFilePath : null,
        hasEmbeddedAudio,
        audioStyle: (editorState?.audioStyle as AudioStyle) ?? null,
        musicTracks: editorState?.musicTracks ?? null,
      };
    }
  );

  ipcMain.handle('capture-preview:get-export-output-path', event => {
    const outputPath = path.join(
      app.getPath('temp'),
      `poratake-clipboard-${randomUUID()}.mp4`
    );
    authorizeExportOutputPaths(event.sender, [outputPath]);
    return outputPath;
  });

  ipcMain.handle(
    'capture-preview:copy-video-to-clipboard',
    async (event, outputPath: string): Promise<boolean> => {
      if (!isExportOutputPathAllowed(event.sender.id, outputPath)) return false;
      if (!existsSync(outputPath)) return false;

      try {
        if (process.platform === 'win32') {
          await writeWindowsFileToClipboard(outputPath);
        } else {
          clipboard.writeBuffer(
            'public.file-url',
            Buffer.from(pathToFileURL(outputPath).href)
          );
        }
        scheduleCleanup(outputPath);
        return true;
      } catch {
        return false;
      }
    }
  );
}

function scheduleCleanup(filePath: string): void {
  setTimeout(() => {
    try {
      if (existsSync(filePath)) {
        unlinkSync(filePath);
      }
    } catch {
      // noop
    }
  }, CLIPBOARD_CLEANUP_MS);
}
