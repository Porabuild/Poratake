import { ipcMain } from 'electron';
import { randomUUID } from 'crypto';
import { getFFmpegPath } from '@/main/utils/ffmpeg';
import type { AudioSegment, AudioSegmentWithSpeed } from '@/types/audio';
import {
  getExportAbortSignal,
  isExportOutputPathAllowed,
} from './export-session';

export function buildAtempoFilter(speed: number): string {
  if (!Number.isFinite(speed) || speed <= 0) {
    throw new Error('Invalid audio speed');
  }

  if (speed === 1) return '';

  const filters: string[] = [];
  let remaining = speed;

  if (speed > 1) {
    while (remaining > 2) {
      filters.push('atempo=2.0');
      remaining /= 2;
    }
    filters.push(`atempo=${remaining}`);
  } else {
    while (remaining < 0.5) {
      filters.push('atempo=0.5');
      remaining /= 0.5;
    }
    filters.push(`atempo=${remaining}`);
  }

  return filters.join(',');
}

function isValidAudioSegment(segment: AudioSegment): boolean {
  return (
    Number.isFinite(segment.start) &&
    Number.isFinite(segment.end) &&
    segment.start >= 0 &&
    segment.end > segment.start
  );
}

export function registerAudioHandlers(): void {
  ipcMain.handle(
    'video-editor:extract-audio',
    async (
      event,
      { inputPath, outputPath }: { inputPath: string; outputPath: string }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);
        const signal = getExportAbortSignal(event.sender.id);

        await execFileAsync(
          ffmpegPath,
          ['-i', inputPath, '-vn', '-acodec', 'copy', '-y', outputPath],
          { signal }
        );

        return { success: true };
      } catch (error) {
        const { unlink } = await import('fs/promises');
        await unlink(outputPath).catch(() => {});
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:extract-audio-segments',
    async (
      event,
      {
        inputPath,
        outputPath,
        segments,
      }: { inputPath: string; outputPath: string; segments: AudioSegment[] }
    ): Promise<{ success: boolean; error?: string }> => {
      const tempFiles: string[] = [];
      let concatListPath: string | null = null;

      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const { writeFile } = await import('fs/promises');
        const { dirname } = await import('path');
        const execFileAsync = promisify(execFile);
        const signal = getExportAbortSignal(event.sender.id);

        if (segments.length === 0) {
          return { success: false, error: 'No segments provided' };
        }
        if (!segments.every(isValidAudioSegment)) {
          return { success: false, error: 'Invalid audio segment' };
        }

        if (segments.length === 1) {
          const seg = segments[0];
          await execFileAsync(
            ffmpegPath,
            [
              '-i',
              inputPath,
              '-ss',
              seg.start.toString(),
              '-to',
              seg.end.toString(),
              '-vn',
              '-acodec',
              'aac',
              '-y',
              outputPath,
            ],
            { signal }
          );
          return { success: true };
        }

        const tempDir = dirname(outputPath);
        const operationId = randomUUID();
        for (let i = 0; i < segments.length; i++) {
          const seg = segments[i];
          const tempFile = `${tempDir}/poratake-audio-${operationId}-${i}.aac`;
          tempFiles.push(tempFile);

          await execFileAsync(
            ffmpegPath,
            [
              '-i',
              inputPath,
              '-ss',
              seg.start.toString(),
              '-to',
              seg.end.toString(),
              '-vn',
              '-acodec',
              'aac',
              '-y',
              tempFile,
            ],
            { signal }
          );
        }

        concatListPath = `${tempDir}/poratake-audio-${operationId}.txt`;
        const concatContent = tempFiles
          .map(f => `file '${f.replace(/'/g, "'\\''")}'`)
          .join('\n');
        await writeFile(concatListPath, concatContent);

        await execFileAsync(
          ffmpegPath,
          [
            '-f',
            'concat',
            '-safe',
            '0',
            '-i',
            concatListPath,
            '-c',
            'copy',
            '-y',
            outputPath,
          ],
          { signal }
        );

        return { success: true };
      } catch (error) {
        const { unlink } = await import('fs/promises');
        await unlink(outputPath).catch(() => {});
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      } finally {
        const { unlink } = await import('fs/promises');
        for (const tempFile of tempFiles) {
          await unlink(tempFile).catch(() => {});
        }
        if (concatListPath) {
          await unlink(concatListPath).catch(() => {});
        }
      }
    }
  );

  ipcMain.handle(
    'video-editor:mux-audio',
    async (
      event,
      {
        videoPath,
        audioPath,
        outputPath,
        audioDelaySeconds = 0,
      }: {
        videoPath: string;
        audioPath: string;
        outputPath: string;
        audioDelaySeconds?: number;
      }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        if (!Number.isFinite(audioDelaySeconds) || audioDelaySeconds < 0) {
          return { success: false, error: 'Invalid audio delay' };
        }

        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);
        const signal = getExportAbortSignal(event.sender.id);

        const args = ['-i', videoPath];
        if (audioDelaySeconds > 0) {
          args.push('-itsoffset', audioDelaySeconds.toString());
        }
        args.push(
          '-i',
          audioPath,
          '-c:v',
          'copy',
          '-c:a',
          'aac',
          '-af',
          'apad',
          '-shortest',
          '-y',
          outputPath
        );

        await execFileAsync(ffmpegPath, args, { signal });

        return { success: true };
      } catch (error) {
        const { unlink } = await import('fs/promises');
        await unlink(outputPath).catch(() => {});
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:mix-audio-tracks',
    async (
      event,
      {
        inputPaths,
        outputPath,
        volumes,
      }: { inputPaths: string[]; outputPath: string; volumes?: number[] }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        if (inputPaths.length === 0) {
          return { success: false, error: 'No audio tracks provided' };
        }
        if (
          volumes &&
          volumes.some(volume => !Number.isFinite(volume) || volume < 0)
        ) {
          return { success: false, error: 'Invalid audio volume' };
        }

        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);
        const signal = getExportAbortSignal(event.sender.id);

        const inputArgs: string[] = [];
        for (const inputPath of inputPaths) {
          inputArgs.push('-i', inputPath);
        }

        let filterComplex: string;
        if (volumes && volumes.some(v => v !== 1)) {
          const volumeFilters = inputPaths
            .map((_, i) => `[${i}:a]volume=${volumes[i] ?? 1}[a${i}]`)
            .join(';');
          const mixInputs = inputPaths.map((_, i) => `[a${i}]`).join('');
          filterComplex = `${volumeFilters};${mixInputs}amix=inputs=${inputPaths.length}:duration=longest`;
        } else {
          filterComplex = `amix=inputs=${inputPaths.length}:duration=longest`;
        }

        await execFileAsync(
          ffmpegPath,
          [
            ...inputArgs,
            '-filter_complex',
            filterComplex,
            '-c:a',
            'aac',
            '-y',
            outputPath,
          ],
          { signal }
        );

        return { success: true };
      } catch (error) {
        const { unlink } = await import('fs/promises');
        await unlink(outputPath).catch(() => {});
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:adjust-audio-volume',
    async (
      event,
      {
        inputPath,
        outputPath,
        volume,
      }: { inputPath: string; outputPath: string; volume: number }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        if (!Number.isFinite(volume) || volume < 0) {
          return { success: false, error: 'Invalid audio volume' };
        }

        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);
        const signal = getExportAbortSignal(event.sender.id);

        await execFileAsync(
          ffmpegPath,
          [
            '-i',
            inputPath,
            '-af',
            `volume=${volume}`,
            '-c:a',
            'aac',
            '-y',
            outputPath,
          ],
          { signal }
        );

        return { success: true };
      } catch (error) {
        const { unlink } = await import('fs/promises');
        await unlink(outputPath).catch(() => {});
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:extract-audio-segments-with-speed',
    async (
      event,
      {
        inputPath,
        outputPath,
        segments,
      }: {
        inputPath: string;
        outputPath: string;
        segments: AudioSegmentWithSpeed[];
      }
    ): Promise<{ success: boolean; error?: string }> => {
      const tempFiles: string[] = [];
      let concatListPath: string | null = null;

      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const { writeFile } = await import('fs/promises');
        const { dirname } = await import('path');
        const execFileAsync = promisify(execFile);
        const signal = getExportAbortSignal(event.sender.id);

        if (segments.length === 0) {
          return { success: false, error: 'No segments provided' };
        }
        if (
          !segments.every(
            segment =>
              isValidAudioSegment(segment) &&
              Number.isFinite(segment.speed) &&
              segment.speed > 0
          )
        ) {
          return { success: false, error: 'Invalid audio segment' };
        }

        const tempDir = dirname(outputPath);
        const operationId = randomUUID();
        for (let i = 0; i < segments.length; i++) {
          const seg = segments[i];
          const tempFile = `${tempDir}/poratake-audio-speed-${operationId}-${i}.aac`;
          tempFiles.push(tempFile);

          const atempoFilter = buildAtempoFilter(seg.speed);
          const hasSpeedChange = seg.speed !== 1;

          const args = [
            '-i',
            inputPath,
            '-ss',
            seg.start.toString(),
            '-to',
            seg.end.toString(),
            '-vn',
          ];

          if (hasSpeedChange) {
            args.push('-af', atempoFilter);
          }

          args.push('-acodec', 'aac', '-y', tempFile);

          await execFileAsync(ffmpegPath, args, { signal });
        }

        if (tempFiles.length === 1) {
          const { rename } = await import('fs/promises');
          await rename(tempFiles[0], outputPath);
          return { success: true };
        }

        concatListPath = `${tempDir}/poratake-audio-speed-${operationId}.txt`;
        const concatContent = tempFiles
          .map(f => `file '${f.replace(/'/g, "'\\''")}'`)
          .join('\n');
        await writeFile(concatListPath, concatContent);

        await execFileAsync(
          ffmpegPath,
          [
            '-f',
            'concat',
            '-safe',
            '0',
            '-i',
            concatListPath,
            '-c',
            'copy',
            '-y',
            outputPath,
          ],
          { signal }
        );

        return { success: true };
      } catch (error) {
        const { unlink } = await import('fs/promises');
        await unlink(outputPath).catch(() => {});
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      } finally {
        const { unlink } = await import('fs/promises');
        for (const tempFile of tempFiles) {
          await unlink(tempFile).catch(() => {});
        }
        if (concatListPath) {
          await unlink(concatListPath).catch(() => {});
        }
      }
    }
  );
}
