import { ipcMain } from 'electron';
import { getFFmpegPath } from '@/main/utils/ffmpeg';
import { getPublicAssetPath } from '@/main/utils/paths';
import type { KeyboardSoundType } from '@/types/audio';
import { KEYBOARD_SOUND_SAMPLES_PER_TYPE } from '@/types/audio';

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
  outputPath: string
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
    { maxBuffer: 50 * 1024 * 1024 }
  );
}

export function registerKeyboardSoundHandlers(): void {
  ipcMain.handle(
    'video-editor:generate-keyboard-audio',
    async (
      _,
      {
        keyPresses,
        soundType,
        duration,
        outputPath,
      }: GenerateKeyboardAudioParams
    ): Promise<{ success: boolean; error?: string }> => {
      try {
        if (keyPresses.length === 0) {
          return { success: false, error: 'No key presses provided' };
        }

        const ffmpegPath = getFFmpegPath();

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
            outputPath
          );
          return { success: true };
        }

        const { execFile } = await import('child_process');
        const { promisify } = await import('util');
        const { unlink } = await import('fs/promises');
        const { dirname } = await import('path');
        const execFileAsync = promisify(execFile);

        const tempDir = dirname(outputPath);
        const chunks: KeyPressEvent[][] = [];
        for (let i = 0; i < keyPresses.length; i += MAX_AMIX_INPUTS) {
          chunks.push(keyPresses.slice(i, i + MAX_AMIX_INPUTS));
        }

        const chunkPaths: string[] = [];
        for (let c = 0; c < chunks.length; c++) {
          const chunkPath = `${tempDir}/temp_kb_chunk_${c}.aac`;
          chunkPaths.push(chunkPath);
          await generateChunk(
            ffmpegPath,
            samplePaths,
            chunks[c],
            duration,
            chunkPath
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
          { maxBuffer: 50 * 1024 * 1024 }
        );

        for (const p of chunkPaths) {
          await unlink(p).catch(() => {});
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
