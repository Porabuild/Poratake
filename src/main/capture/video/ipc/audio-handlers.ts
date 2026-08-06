import { ipcMain } from 'electron';
import { getFFmpegPath } from '@/main/utils/ffmpeg';
import type { AudioSegment, AudioSegmentWithSpeed } from '@/types/audio';

function buildAtempoFilter(speed: number): string {
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

export function registerAudioHandlers(): void {
  ipcMain.handle(
    'video-editor:extract-audio',
    async (
      _,
      { inputPath, outputPath }: { inputPath: string; outputPath: string }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);

        await execFileAsync(ffmpegPath, [
          '-i',
          inputPath,
          '-vn',
          '-acodec',
          'copy',
          '-y',
          outputPath,
        ]);

        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:extract-audio-segments',
    async (
      _,
      {
        inputPath,
        outputPath,
        segments,
      }: { inputPath: string; outputPath: string; segments: AudioSegment[] }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const { writeFile, unlink } = await import('fs/promises');
        const { dirname } = await import('path');
        const execFileAsync = promisify(execFile);

        if (segments.length === 0) {
          return { success: false, error: 'No segments provided' };
        }

        if (segments.length === 1) {
          const seg = segments[0];
          await execFileAsync(ffmpegPath, [
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
          ]);
          return { success: true };
        }

        const tempDir = dirname(outputPath);
        const tempFiles: string[] = [];

        for (let i = 0; i < segments.length; i++) {
          const seg = segments[i];
          const tempFile = `${tempDir}/temp_audio_seg_${i}.aac`;
          tempFiles.push(tempFile);

          await execFileAsync(ffmpegPath, [
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
          ]);
        }

        const concatListPath = `${tempDir}/concat_list.txt`;
        const concatContent = tempFiles.map(f => `file '${f}'`).join('\n');
        await writeFile(concatListPath, concatContent);

        await execFileAsync(ffmpegPath, [
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
        ]);

        for (const tempFile of tempFiles) {
          await unlink(tempFile).catch(() => {});
        }
        await unlink(concatListPath).catch(() => {});

        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:mux-audio',
    async (
      _,
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
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);

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
          '-y',
          outputPath
        );

        await execFileAsync(ffmpegPath, args);

        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:mix-audio-tracks',
    async (
      _,
      {
        inputPaths,
        outputPath,
        volumes,
      }: { inputPaths: string[]; outputPath: string; volumes?: number[] }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);

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

        await execFileAsync(ffmpegPath, [
          ...inputArgs,
          '-filter_complex',
          filterComplex,
          '-c:a',
          'aac',
          '-y',
          outputPath,
        ]);

        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:adjust-audio-volume',
    async (
      _,
      {
        inputPath,
        outputPath,
        volume,
      }: { inputPath: string; outputPath: string; volume: number }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const execFileAsync = promisify(execFile);

        await execFileAsync(ffmpegPath, [
          '-i',
          inputPath,
          '-af',
          `volume=${volume}`,
          '-c:a',
          'aac',
          '-y',
          outputPath,
        ]);

        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );

  ipcMain.handle(
    'video-editor:extract-audio-segments-with-speed',
    async (
      _,
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
      try {
        const ffmpegPath = getFFmpegPath();
        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const { writeFile, unlink } = await import('fs/promises');
        const { dirname } = await import('path');
        const execFileAsync = promisify(execFile);

        if (segments.length === 0) {
          return { success: false, error: 'No segments provided' };
        }

        const tempDir = dirname(outputPath);
        const tempFiles: string[] = [];

        for (let i = 0; i < segments.length; i++) {
          const seg = segments[i];
          const tempFile = `${tempDir}/temp_audio_speed_seg_${i}.aac`;
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

          await execFileAsync(ffmpegPath, args);
        }

        if (tempFiles.length === 1) {
          const { rename } = await import('fs/promises');
          await rename(tempFiles[0], outputPath);
          return { success: true };
        }

        const concatListPath = `${tempDir}/concat_speed_list.txt`;
        const concatContent = tempFiles.map(f => `file '${f}'`).join('\n');
        await writeFile(concatListPath, concatContent);

        await execFileAsync(ffmpegPath, [
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
        ]);

        for (const tempFile of tempFiles) {
          await unlink(tempFile).catch(() => {});
        }
        await unlink(concatListPath).catch(() => {});

        return { success: true };
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        return { success: false, error: errorMessage };
      }
    }
  );
}
