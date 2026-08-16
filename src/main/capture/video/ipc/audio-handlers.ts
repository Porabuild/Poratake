import { ipcMain } from 'electron';
import { getFFmpegPath } from '@/main/utils/ffmpeg';
import {
  execFFmpegFile,
  spawnFFmpegProcess,
} from '@/main/utils/ffmpeg-process';
import type { AudioSegment, AudioSegmentWithSpeed } from '@/types/audio';
import {
  getExportAbortSignal,
  isExportOutputPathAllowed,
} from './export-session';

interface MuxAudioTrack {
  path: string;
  volume?: number;
}

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

export function buildAudioSegmentsFilter(
  segments: AudioSegmentWithSpeed[]
): string {
  const chains: string[] = [];
  const labels: string[] = [];

  segments.forEach((segment, index) => {
    const parts = [`atrim=start=${segment.start}:end=${segment.end}`];

    const atempoFilter = buildAtempoFilter(segment.speed ?? 1);
    if (atempoFilter) {
      parts.push(atempoFilter);
    }

    parts.push('asetpts=PTS-STARTPTS');

    if (segments.length === 1) {
      chains.push(`[0:a]${parts.join(',')}[aout]`);
      return;
    }

    chains.push(`[s${index}]${parts.join(',')}[c${index}]`);
    labels.push(`[c${index}]`);
  });

  if (segments.length > 1) {
    chains.push(
      `[0:a]asplit=${segments.length}${segments
        .map((_, index) => `[s${index}]`)
        .join('')}`
    );
    chains.push(`${labels.join('')}concat=n=${segments.length}:v=0:a=1[aout]`);
  }

  return chains.join(';');
}

export function buildMuxAudioFilter(
  trackCount: number,
  volumes: number[],
  audioDelaySeconds: number
): string | null {
  const needsVolume = volumes.some(volume => volume !== 1);
  const delayMs = Math.round(audioDelaySeconds * 1000);

  if (trackCount === 1 && !needsVolume && delayMs === 0) {
    return null;
  }

  const filters: string[] = [];
  const labels: string[] = [];

  for (let index = 0; index < trackCount; index++) {
    const volume = volumes[index] ?? 1;
    if (volume === 1) {
      labels.push(`[${index + 1}:a]`);
      continue;
    }
    filters.push(`[${index + 1}:a]volume=${volume}[v${index}]`);
    labels.push(`[v${index}]`);
  }

  let head: string;
  if (trackCount > 1) {
    filters.push(
      `${labels.join('')}amix=inputs=${trackCount}:duration=longest[mix]`
    );
    head = '[mix]';
  } else {
    head = labels[0];
  }

  const tail: string[] = [];
  if (delayMs > 0) {
    tail.push(`adelay=${delayMs}:all=1`);
  }
  tail.push('apad');

  filters.push(`${head}${tail.join(',')}[aout]`);

  return filters.join(';');
}

function parseProgressSeconds(line: string): number | null {
  const match = /^out_time_(?:us|ms)=(\d+)$/.exec(line.trim());
  if (!match) return null;

  return Number(match[1]) / 1_000_000;
}

async function runFFmpegWithProgress(
  ffmpegPath: string,
  args: string[],
  signal: AbortSignal | undefined,
  onOutSeconds: (outSeconds: number) => void
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const child = spawnFFmpegProcess(
      ffmpegPath,
      ['-progress', 'pipe:1', '-nostats', ...args],
      { signal }
    );

    let stderr = '';
    let pending = '';
    let settled = false;

    const settle = (error?: Error): void => {
      if (settled) return;
      settled = true;
      signal?.removeEventListener('abort', abort);
      if (error) {
        reject(error);
        return;
      }
      resolve();
    };

    function abort(): void {
      child.kill('SIGKILL');
    }

    if (signal?.aborted) {
      abort();
      return;
    }
    signal?.addEventListener('abort', abort);

    child.stdout?.on('data', (chunk: Buffer | string) => {
      pending += chunk.toString();
      const lines = pending.split('\n');
      pending = lines.pop() ?? '';

      for (const line of lines) {
        const outSeconds = parseProgressSeconds(line);
        if (outSeconds === null) continue;
        onOutSeconds(outSeconds);
      }
    });

    child.stderr?.on('data', (chunk: Buffer | string) => {
      stderr += chunk.toString();
    });

    child.on('error', error => settle(error));

    child.on('close', code => {
      if (signal?.aborted) {
        settle(new Error('The operation was aborted'));
        return;
      }
      if (code === 0) {
        settle();
        return;
      }
      settle(
        new Error(`ffmpeg exited with code ${code}: ${stderr.trim()}`.trim())
      );
    });
  });
}

async function runFFmpeg(
  args: string[],
  signal: AbortSignal | undefined
): Promise<void> {
  await execFFmpegFile(getFFmpegPath(), args, { signal });
}

function isValidAudioSegment(segment: AudioSegment): boolean {
  return (
    Number.isFinite(segment.start) &&
    Number.isFinite(segment.end) &&
    segment.start >= 0 &&
    segment.end > segment.start
  );
}

async function extractAudioSegmentsInOnePass(
  event: Electron.IpcMainInvokeEvent,
  inputPath: string,
  outputPath: string,
  segments: AudioSegmentWithSpeed[]
): Promise<{ success: boolean; error?: string }> {
  try {
    if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
      return { success: false, error: 'Export path is not authorized' };
    }

    await runFFmpeg(
      [
        '-i',
        inputPath,
        '-filter_complex',
        buildAudioSegmentsFilter(segments),
        '-map',
        '[aout]',
        '-acodec',
        'aac',
        '-y',
        outputPath,
      ],
      getExportAbortSignal(event.sender.id)
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

        await runFFmpeg(
          ['-i', inputPath, '-vn', '-acodec', 'copy', '-y', outputPath],
          getExportAbortSignal(event.sender.id)
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
      if (segments.length === 0) {
        return { success: false, error: 'No segments provided' };
      }
      if (!segments.every(isValidAudioSegment)) {
        return { success: false, error: 'Invalid audio segment' };
      }

      return await extractAudioSegmentsInOnePass(
        event,
        inputPath,
        outputPath,
        segments.map(segment => ({ ...segment, speed: 1 }))
      );
    }
  );

  ipcMain.handle(
    'video-editor:mux-audio',
    async (
      event,
      {
        videoPath,
        audioTracks,
        outputPath,
        audioDelaySeconds = 0,
        durationSeconds,
      }: {
        videoPath: string;
        audioTracks: MuxAudioTrack[];
        outputPath: string;
        audioDelaySeconds?: number;
        durationSeconds?: number;
      }
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        if (
          audioTracks.length === 0 ||
          audioTracks.some(
            track =>
              !track.path ||
              !Number.isFinite(track.volume ?? 1) ||
              (track.volume ?? 1) < 0
          )
        ) {
          return { success: false, error: 'Invalid audio tracks' };
        }
        if (!Number.isFinite(audioDelaySeconds) || audioDelaySeconds < 0) {
          return { success: false, error: 'Invalid audio delay' };
        }

        const signal = getExportAbortSignal(event.sender.id);
        const totalSeconds =
          typeof durationSeconds === 'number' &&
          Number.isFinite(durationSeconds) &&
          durationSeconds > 0
            ? durationSeconds
            : null;

        const volumes = audioTracks.map(track => track.volume ?? 1);
        const filterComplex = buildMuxAudioFilter(
          audioTracks.length,
          volumes,
          audioDelaySeconds
        );

        const args = ['-i', videoPath];
        for (const track of audioTracks) {
          args.push('-i', track.path);
        }

        if (filterComplex) {
          args.push(
            '-filter_complex',
            filterComplex,
            '-map',
            '0:v',
            '-map',
            '[aout]'
          );
        } else {
          args.push('-map', '0:v', '-map', '1:a', '-af', 'apad');
        }

        args.push('-c:v', 'copy', '-c:a', 'aac');

        if (totalSeconds !== null) {
          args.push('-t', totalSeconds.toString());
        } else {
          args.push('-shortest');
        }

        args.push('-y', outputPath);

        let lastPercent = -1;
        await runFFmpegWithProgress(
          getFFmpegPath(),
          args,
          signal,
          outSeconds => {
            if (totalSeconds === null) return;

            const percent = Math.min(
              100,
              Math.round((outSeconds / totalSeconds) * 100)
            );
            if (percent === lastPercent) return;

            lastPercent = percent;
            event.sender.send('video-editor:mux-audio:progress', percent);
          }
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
      if (segments.length === 0) {
        return { success: false, error: 'No segments provided' };
      }
      if (
        !segments.every(
          segment =>
            isValidAudioSegment(segment) &&
            Number.isFinite(segment.speed ?? 1) &&
            (segment.speed ?? 1) > 0
        )
      ) {
        return { success: false, error: 'Invalid audio segment' };
      }

      return await extractAudioSegmentsInOnePass(
        event,
        inputPath,
        outputPath,
        segments
      );
    }
  );
}
