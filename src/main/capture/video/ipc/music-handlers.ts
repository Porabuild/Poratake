import { ipcMain, dialog, BrowserWindow } from 'electron';
import { existsSync } from 'fs';
import fs from 'fs/promises';
import path from 'path';
import { execFile } from 'child_process';
import { promisify } from 'util';
import { getWindowData } from '../window-manager';
import { getMusicFolderPath } from '../recording-project';
import { getFFmpegPath } from '@/main/utils/ffmpeg';
import { SUPPORTED_MUSIC_EXTENSIONS } from '@/types/music';

const execFileAsync = promisify(execFile);

function sanitizeFileName(fileName: string): string {
  return path.basename(fileName);
}

async function probeAudioDuration(filePath: string): Promise<number> {
  const ffmpegPath = getFFmpegPath();

  let stderr = '';
  try {
    await execFileAsync(ffmpegPath, ['-i', filePath], { timeout: 10000 });
  } catch (error) {
    stderr = (error as { stderr?: string }).stderr || '';
  }

  const durationMatch = stderr.match(
    /Duration:\s*(\d{2}):(\d{2}):(\d{2})\.(\d{2})/
  );
  if (!durationMatch) return 0;

  const hours = parseInt(durationMatch[1]);
  const minutes = parseInt(durationMatch[2]);
  const seconds = parseInt(durationMatch[3]);
  const centiseconds = parseInt(durationMatch[4]);
  return hours * 3600 + minutes * 60 + seconds + centiseconds / 100;
}

async function getUniqueFileName(
  musicFolder: string,
  originalName: string
): Promise<string> {
  const ext = path.extname(originalName);
  const base = path.basename(originalName, ext);
  let candidate = originalName;
  let counter = 1;

  while (existsSync(path.join(musicFolder, candidate))) {
    candidate = `${base} (${counter})${ext}`;
    counter++;
  }

  return candidate;
}

export function registerMusicHandlers(): void {
  ipcMain.handle(
    'video-editor:music:add',
    async (
      event
    ): Promise<{
      success: boolean;
      fileName?: string;
      name?: string;
      originalDuration?: number;
      error?: string;
    }> => {
      const data = getWindowData(event.sender.id);
      if (!data) {
        return { success: false, error: 'No video data found' };
      }

      const window = BrowserWindow.fromWebContents(event.sender);
      if (!window) {
        return { success: false, error: 'Window not found' };
      }

      const result = await dialog.showOpenDialog(window, {
        title: 'Add Music',
        filters: [
          {
            name: 'Audio Files',
            extensions: SUPPORTED_MUSIC_EXTENSIONS,
          },
        ],
        properties: ['openFile'],
      });

      if (result.canceled || result.filePaths.length === 0) {
        return { success: false, error: 'Cancelled' };
      }

      const sourcePath = result.filePaths[0];
      const musicFolder = getMusicFolderPath(data.filePath);
      if (!musicFolder) {
        return { success: false, error: 'Could not resolve music folder' };
      }

      if (!existsSync(musicFolder)) {
        await fs.mkdir(musicFolder, { recursive: true });
      }

      const originalName = path.basename(sourcePath);
      const fileName = await getUniqueFileName(musicFolder, originalName);
      const destPath = path.join(musicFolder, fileName);

      try {
        await fs.copyFile(sourcePath, destPath);
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : 'Failed to copy file',
        };
      }

      const duration = await probeAudioDuration(destPath);
      if (duration <= 0) {
        await fs.unlink(destPath).catch(() => {});
        return { success: false, error: 'Could not determine audio duration' };
      }

      const ext = path.extname(fileName);
      const name = path.basename(fileName, ext);

      return { success: true, fileName, name, originalDuration: duration };
    }
  );

  ipcMain.handle(
    'video-editor:music:remove',
    async (
      event,
      { fileName }: { fileName: string }
    ): Promise<{ success: boolean; error?: string }> => {
      const data = getWindowData(event.sender.id);
      if (!data) {
        return { success: false, error: 'No video data found' };
      }

      const musicFolder = getMusicFolderPath(data.filePath);
      if (!musicFolder) {
        return { success: false, error: 'Could not resolve music folder' };
      }

      const safeFileName = sanitizeFileName(fileName);
      const filePath = path.join(musicFolder, safeFileName);
      try {
        await fs.unlink(filePath);
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
          return { success: true };
        }
        return {
          success: false,
          error:
            error instanceof Error ? error.message : 'Failed to remove file',
        };
      }
      return { success: true };
    }
  );

  ipcMain.handle(
    'video-editor:music:get-path',
    async (
      event,
      { fileName }: { fileName: string }
    ): Promise<string | null> => {
      const data = getWindowData(event.sender.id);
      if (!data) return null;

      const musicFolder = getMusicFolderPath(data.filePath);
      if (!musicFolder) return null;

      const safeFileName = sanitizeFileName(fileName);
      const filePath = path.join(musicFolder, safeFileName);
      return existsSync(filePath) ? filePath : null;
    }
  );

  ipcMain.handle(
    'video-editor:music:prepare-for-export',
    async (
      _,
      {
        musicFilePath,
        trimStart,
        trimEnd,
        speed,
        startTime,
        trackDuration,
        totalDuration,
        outputPath,
      }: {
        musicFilePath: string;
        trimStart: number;
        trimEnd: number;
        speed: number;
        startTime: number;
        trackDuration?: number;
        totalDuration: number;
        outputPath: string;
      }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        const ffmpegPath = getFFmpegPath();
        const duration = await probeAudioDuration(musicFilePath);
        const trimmedEnd = duration - trimEnd;
        const trimmedDuration = (trimmedEnd - trimStart) / speed;

        const clippedDuration = Math.min(
          trimmedDuration,
          totalDuration - startTime,
          ...(trackDuration !== undefined ? [trackDuration] : [])
        );

        if (clippedDuration <= 0) {
          return { success: false, error: 'Music track has no audible range' };
        }

        const atempoFilters: string[] = [];
        let remaining = speed;
        if (speed > 1) {
          while (remaining > 2) {
            atempoFilters.push('atempo=2.0');
            remaining /= 2;
          }
          atempoFilters.push(`atempo=${remaining}`);
        } else if (speed < 1) {
          while (remaining < 0.5) {
            atempoFilters.push('atempo=0.5');
            remaining /= 0.5;
          }
          atempoFilters.push(`atempo=${remaining}`);
        }

        if (startTime > 0) {
          const args = [
            '-f',
            'lavfi',
            '-t',
            startTime.toString(),
            '-i',
            `anullsrc=r=44100:cl=stereo`,
            '-ss',
            trimStart.toString(),
            '-to',
            trimmedEnd.toString(),
            '-i',
            musicFilePath,
          ];

          const audioFilters: string[] = [];
          if (atempoFilters.length > 0) {
            audioFilters.push(
              `[1:a]${atempoFilters.join(',')}[sped]`,
              `[0:a][sped]concat=n=2:v=0:a=1`
            );
          } else {
            audioFilters.push(`[0:a][1:a]concat=n=2:v=0:a=1`);
          }

          args.push(
            '-filter_complex',
            audioFilters.join(';'),
            '-t',
            (startTime + clippedDuration).toString(),
            '-acodec',
            'aac',
            '-y',
            outputPath
          );

          await execFileAsync(ffmpegPath, args, { timeout: 120000 });
        } else {
          const args = [
            '-ss',
            trimStart.toString(),
            '-to',
            trimmedEnd.toString(),
            '-i',
            musicFilePath,
            '-vn',
          ];

          if (atempoFilters.length > 0) {
            args.push('-af', atempoFilters.join(','));
          }

          args.push(
            '-t',
            clippedDuration.toString(),
            '-acodec',
            'aac',
            '-y',
            outputPath
          );

          await execFileAsync(ffmpegPath, args, { timeout: 120000 });
        }

        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );
}
