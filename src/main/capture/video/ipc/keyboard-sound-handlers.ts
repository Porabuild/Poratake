import { ipcMain } from 'electron';
import { randomUUID } from 'crypto';
import { getFFmpegPath } from '@/main/utils/ffmpeg';
import { getPublicAssetPath } from '@/main/utils/paths';
import type { KeyboardSoundType } from '@/types/audio';
import {
  KEYBOARD_SOUND_OPTIONS,
  KEYBOARD_SOUND_SAMPLES_PER_TYPE,
} from '@/types/audio';
import {
  getExportAbortSignal,
  isExportOutputPathAllowed,
} from './export-session';

interface KeyPressEvent {
  timestamp: number;
}

interface GenerateKeyboardAudioParams {
  keyPresses: KeyPressEvent[];
  soundType: KeyboardSoundType;
  duration: number;
  outputPath: string;
}

function getSamplePath(soundType: KeyboardSoundType, index: number): string {
  return getPublicAssetPath(
    `sounds/keyboard/${soundType}/press-${index + 1}.mp3`
  );
}

function getSampleIndexForTimestamp(timestamp: number, count: number): number {
  const hash = Math.round(timestamp * 1000) * 2654435761;
  return Math.abs(hash) % count;
}

const MAX_AMIX_INPUTS = 50;

async function generateChunk(
  ffmpegPath: string,
  samplePaths: string[],
  keyPresses: KeyPressEvent[],
  duration: number,
  outputPath: string,
  signal?: AbortSignal
): Promise<void> {
  const { execFile } = await import('child_process');
  const { promisify } = await import('util');
  const execFileAsync = promisify(execFile);

  const sampleCount = samplePaths.length;
  const inputArgs: string[] = [];
  const filterParts: string[] = [];

  for (let i = 0; i < keyPresses.length; i++) {
    const sampleIdx = getSampleIndexForTimestamp(
      keyPresses[i].timestamp,
      sampleCount
    );
    inputArgs.push('-i', samplePaths[sampleIdx]);
  }

  const silenceIdx = keyPresses.length;
  inputArgs.push(
    '-f',
    'lavfi',
    '-i',
    `anullsrc=channel_layout=stereo:sample_rate=44100`,
    '-t',
    duration.toString()
  );

  for (let i = 0; i < keyPresses.length; i++) {
    const delayMs = Math.max(0, Math.round(keyPresses[i].timestamp * 1000));
    filterParts.push(`[${i}]adelay=${delayMs}|${delayMs}[d${i}]`);
  }

  const mixInputs = keyPresses.map((_, i) => `[d${i}]`).join('');
  const mixFilter = `${mixInputs}[${silenceIdx}]amix=inputs=${keyPresses.length + 1}:duration=longest:normalize=0`;
  const filterComplex = `${filterParts.join(';')};${mixFilter}`;

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
    { maxBuffer: 50 * 1024 * 1024, signal }
  );
}

export function registerKeyboardSoundHandlers(): void {
  ipcMain.handle(
    'video-editor:generate-keyboard-audio',
    async (
      event,
      {
        keyPresses,
        soundType,
        duration,
        outputPath,
      }: GenerateKeyboardAudioParams
    ): Promise<{ success: boolean; error?: string }> => {
      const chunkPaths: string[] = [];

      try {
        if (!isExportOutputPathAllowed(event.sender.id, outputPath)) {
          return { success: false, error: 'Export path is not authorized' };
        }
        if (keyPresses.length === 0) {
          return { success: false, error: 'No key presses provided' };
        }
        if (!Number.isFinite(duration) || duration <= 0) {
          return { success: false, error: 'Invalid keyboard audio duration' };
        }
        if (
          !keyPresses.every(
            keyPress =>
              Number.isFinite(keyPress.timestamp) &&
              keyPress.timestamp >= 0 &&
              keyPress.timestamp < duration
          )
        ) {
          return { success: false, error: 'Invalid key press timestamp' };
        }
        if (
          !KEYBOARD_SOUND_OPTIONS.some(option => option.value === soundType)
        ) {
          return { success: false, error: 'Invalid keyboard sound type' };
        }

        const ffmpegPath = getFFmpegPath();
        const signal = getExportAbortSignal(event.sender.id);

        const samplePaths: string[] = [];
        for (let i = 0; i < KEYBOARD_SOUND_SAMPLES_PER_TYPE; i++) {
          samplePaths.push(getSamplePath(soundType, i));
        }

        if (keyPresses.length <= MAX_AMIX_INPUTS) {
          await generateChunk(
            ffmpegPath,
            samplePaths,
            keyPresses,
            duration,
            outputPath,
            signal
          );
          return { success: true };
        }

        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const { dirname } = await import('path');
        const execFileAsync = promisify(execFile);

        const tempDir = dirname(outputPath);
        const operationId = randomUUID();
        const chunks: KeyPressEvent[][] = [];
        for (let i = 0; i < keyPresses.length; i += MAX_AMIX_INPUTS) {
          chunks.push(keyPresses.slice(i, i + MAX_AMIX_INPUTS));
        }

        for (let c = 0; c < chunks.length; c++) {
          const chunkPath = `${tempDir}/poratake-keyboard-${operationId}-${c}.aac`;
          chunkPaths.push(chunkPath);
          await generateChunk(
            ffmpegPath,
            samplePaths,
            chunks[c],
            duration,
            chunkPath,
            signal
          );
        }

        if (chunkPaths.length === 1) {
          const { rename } = await import('fs/promises');
          await rename(chunkPaths[0], outputPath);
          return { success: true };
        }

        const inputArgs: string[] = [];
        for (const p of chunkPaths) {
          inputArgs.push('-i', p);
        }
        const mixFilter = `amix=inputs=${chunkPaths.length}:duration=longest:normalize=0`;

        await execFileAsync(
          ffmpegPath,
          [
            ...inputArgs,
            '-filter_complex',
            mixFilter,
            '-c:a',
            'aac',
            '-y',
            outputPath,
          ],
          { maxBuffer: 50 * 1024 * 1024, signal }
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
        for (const chunkPath of chunkPaths) {
          await unlink(chunkPath).catch(() => {});
        }
      }
    }
  );
}
