import { spawn } from 'child_process';
import fs from 'fs';
import path from 'path';
import { getWhisperCliPath, getWhisperModelPath } from '../utils/whisper';
import { getFFmpegPath } from '../utils/ffmpeg';
import { execFFmpegFile } from '../utils/ffmpeg-process';
import type {
  SubtitleData,
  SubtitleSegment,
  SubtitleWord,
  SubtitleGenerationOptions,
  WhisperModel,
} from '@/types/subtitle';

interface WhisperJsonSegment {
  timestamps: {
    from: string;
    to: string;
  };
  offsets?: {
    from: number;
    to: number;
  };
  text: string;
  tokens?: WhisperJsonToken[];
}

interface WhisperJsonToken {
  text?: string;
  offsets?: {
    from: number;
    to: number;
  };
  t_dtw?: number;
}

interface WhisperJsonOutput {
  transcription: WhisperJsonSegment[];
}

export function parseWhisperOutput(
  jsonOutput: WhisperJsonOutput
): SubtitleSegment[] {
  if (!Array.isArray(jsonOutput.transcription)) {
    return [];
  }

  return jsonOutput.transcription.map(segment => {
    const start =
      segment.offsets?.from !== undefined
        ? segment.offsets.from / 1000
        : (parseWhisperTimestamp(segment.timestamps.from) ?? 0);
    const end =
      segment.offsets?.to !== undefined
        ? segment.offsets.to / 1000
        : (parseWhisperTimestamp(segment.timestamps.to) ?? start);

    const words = parseWhisperTokens(segment.tokens, start, end);

    return {
      start,
      end,
      text: segment.text.trim(),
      words,
    };
  });
}

function parseWhisperTimestamp(value: string): number | null {
  const match = value.trim().match(/^(\d+):(\d{2}):(\d{2})(?:[.,](\d{1,3}))?$/);
  if (!match) return null;

  const hours = Number.parseInt(match[1], 10);
  const minutes = Number.parseInt(match[2], 10);
  const seconds = Number.parseInt(match[3], 10);
  const millisRaw = match[4] ?? '0';
  const millis = Number.parseInt(millisRaw.padEnd(3, '0'), 10);

  if (Number.isNaN(hours) || Number.isNaN(minutes) || Number.isNaN(seconds)) {
    return null;
  }

  return hours * 3600 + minutes * 60 + seconds + millis / 1000;
}

function cleanTokenText(text: string): string {
  return text.replace(/\[_TT_\d+\]/g, '').trim();
}

function parseWhisperTokens(
  tokens: WhisperJsonToken[] | undefined,
  segmentStart: number,
  segmentEnd: number
): SubtitleWord[] | undefined {
  if (!Array.isArray(tokens) || tokens.length === 0) {
    return undefined;
  }

  const parsedTokens = tokens
    .map(token => {
      const tokenText = token.text ?? '';
      const trimmed = cleanTokenText(tokenText);
      if (!trimmed) {
        return null;
      }

      const offsets = token.offsets;
      const start = offsets ? offsets.from / 1000 : null;
      const end = offsets ? offsets.to / 1000 : null;
      const moment =
        token.t_dtw !== undefined && token.t_dtw > 0 ? token.t_dtw / 100 : null;

      if (start === null && end === null && moment === null) {
        return null;
      }

      return {
        text: trimmed,
        hasLeadingSpace: /^\s/.test(tokenText),
        start,
        end,
        moment,
      };
    })
    .filter(
      (
        token
      ): token is {
        text: string;
        hasLeadingSpace: boolean;
        start: number | null;
        end: number | null;
        moment: number | null;
      } => token !== null
    );

  if (parsedTokens.length === 0) {
    return undefined;
  }

  const dtwMoments = parsedTokens
    .map(token => token.moment)
    .filter((moment): moment is number => moment !== null);
  const hasDtw = dtwMoments.length > 0 && Math.max(...dtwMoments) > 0;
  const previousMoments: Array<number | null> = [];
  const nextMoments: Array<number | null> = new Array(parsedTokens.length).fill(
    null
  );

  if (hasDtw) {
    let lastMoment: number | null = null;
    for (const token of parsedTokens) {
      previousMoments.push(lastMoment);
      if (token.moment !== null) {
        lastMoment = token.moment;
      }
    }

    let nextMoment: number | null = null;
    for (let index = parsedTokens.length - 1; index >= 0; index -= 1) {
      nextMoments[index] = nextMoment;
      if (parsedTokens[index].moment !== null) {
        nextMoment = parsedTokens[index].moment;
      }
    }
  } else {
    for (let index = 0; index < parsedTokens.length; index += 1) {
      previousMoments.push(null);
    }
  }

  const timedTokens = parsedTokens
    .map((token, index) => {
      let start = token.start;
      let end = token.end;

      if (hasDtw && token.moment !== null) {
        const previousMoment = previousMoments[index];
        const nextMoment = nextMoments[index];

        start =
          previousMoment !== null
            ? (previousMoment + token.moment) / 2
            : (token.start ?? segmentStart);
        end =
          nextMoment !== null
            ? (token.moment + nextMoment) / 2
            : (token.end ?? segmentEnd);
      }

      if (start === null || end === null) {
        return null;
      }

      if (Number.isNaN(start) || Number.isNaN(end)) {
        return null;
      }

      const clampedStart = Math.max(segmentStart, start);
      const clampedEnd = Math.min(segmentEnd, end);

      if (clampedEnd <= clampedStart) {
        return null;
      }

      return {
        text: token.text,
        hasLeadingSpace: token.hasLeadingSpace,
        start: clampedStart,
        end: clampedEnd,
      };
    })
    .filter(
      (
        token
      ): token is {
        text: string;
        hasLeadingSpace: boolean;
        start: number;
        end: number;
      } => token !== null
    );

  if (timedTokens.length === 0) {
    return undefined;
  }

  const words: SubtitleWord[] = [];
  let currentText = '';
  let currentStart: number | null = null;
  let currentEnd: number | null = null;

  for (const token of timedTokens) {
    if (!currentText || token.hasLeadingSpace) {
      if (currentText && currentStart !== null && currentEnd !== null) {
        words.push({
          text: currentText,
          start: currentStart,
          end: currentEnd,
        });
      }

      currentText = token.text;
      currentStart = token.start;
      currentEnd = token.end;
      continue;
    }

    currentText += token.text;
    currentEnd = token.end;
  }

  if (currentText && currentStart !== null && currentEnd !== null) {
    words.push({
      text: currentText,
      start: currentStart,
      end: currentEnd,
    });
  }

  return words.length > 0 ? words : undefined;
}

function isDtwUnsupported(stderr: string): boolean {
  const lower = stderr.toLowerCase();
  return (
    lower.includes('unknown argument') &&
    (lower.includes('-dtw') || lower.includes('--dtw'))
  );
}

function getDtwPreset(model: WhisperModel): string {
  switch (model) {
    case 'base':
    case 'small':
    case 'medium':
      return model;
  }
}

function runWhisper(
  whisperCliPath: string,
  args: string[],
  onProgress?: (percent: number) => void
): Promise<{ code: number | null; stderr: string }> {
  return new Promise((resolve, reject) => {
    const proc = spawn(whisperCliPath, args);
    let stderr = '';

    proc.stderr.on('data', data => {
      const chunk = data.toString();
      stderr += chunk;

      const progressMatch = chunk.match(/progress\s*=\s*(\d+)%/i);
      if (progressMatch) {
        const whisperProgress = parseInt(progressMatch[1], 10);
        const overallProgress = 15 + Math.round(whisperProgress * 0.8);
        onProgress?.(overallProgress);
      }
    });

    proc.on('close', code => {
      resolve({ code, stderr });
    });

    proc.on('error', reject);
  });
}

async function convertToWav(
  inputPath: string,
  outputPath: string
): Promise<void> {
  const ffmpegPath = getFFmpegPath();

  await execFFmpegFile(ffmpegPath, [
    '-i',
    inputPath,
    '-ar',
    '16000',
    '-ac',
    '1',
    '-c:a',
    'pcm_s16le',
    '-y',
    outputPath,
  ]);
}

const DEFAULT_PROMPT_PREFIX = '';

export interface TranscriptionResult {
  success: boolean;
  data?: SubtitleData;
  error?: string;
}

export async function transcribeAudio(
  audioPath: string,
  options: SubtitleGenerationOptions,
  onProgress?: (percent: number) => void
): Promise<TranscriptionResult> {
  const whisperCliPath = getWhisperCliPath();
  const modelPath = getWhisperModelPath(options.model);

  if (!fs.existsSync(whisperCliPath)) {
    return { success: false, error: 'Whisper binary not found' };
  }

  if (!fs.existsSync(modelPath)) {
    return { success: false, error: `Model ${options.model} not found` };
  }

  if (!fs.existsSync(audioPath)) {
    return { success: false, error: 'Audio file not found' };
  }

  const tempDir = path.dirname(audioPath);
  const wavPath = path.join(tempDir, 'temp_transcribe.wav');
  const jsonPath = path.join(tempDir, 'temp_transcribe.json');

  try {
    onProgress?.(5);

    await convertToWav(audioPath, wavPath);
    onProgress?.(15);

    const prompt = options.prompt
      ? `${DEFAULT_PROMPT_PREFIX} ${options.prompt}`.trim()
      : DEFAULT_PROMPT_PREFIX.trim();

    const args = [
      '-m',
      modelPath,
      '-f',
      wavPath,
      '-ojf',
      '-of',
      jsonPath.replace('.json', ''),
      '-ml',
      '60',
      '-sow',
      '-nfa',
      '-dtw',
      getDtwPreset(options.model),
    ];

    if (prompt) {
      args.push('--prompt', prompt);
    }

    const initialResult = await runWhisper(whisperCliPath, args, onProgress);

    if (initialResult.code !== 0) {
      if (isDtwUnsupported(initialResult.stderr)) {
        const fallbackArgs = [...args];
        const dtwIndex = fallbackArgs.indexOf('-dtw');
        if (dtwIndex !== -1) {
          fallbackArgs.splice(dtwIndex, 2);
        }
        const nfaIndex = fallbackArgs.indexOf('-nfa');
        if (nfaIndex !== -1) {
          fallbackArgs.splice(nfaIndex, 1);
        }
        const jsonIndex = fallbackArgs.indexOf('-ojf');
        if (jsonIndex !== -1) {
          fallbackArgs[jsonIndex] = '-oj';
        }

        const fallbackResult = await runWhisper(
          whisperCliPath,
          fallbackArgs,
          onProgress
        );

        if (fallbackResult.code !== 0) {
          throw new Error(
            `Whisper exited with code ${fallbackResult.code}: ${fallbackResult.stderr}`
          );
        }
      } else {
        throw new Error(
          `Whisper exited with code ${initialResult.code}: ${initialResult.stderr}`
        );
      }
    }

    onProgress?.(95);

    const jsonOutputPath = jsonPath;
    if (!fs.existsSync(jsonOutputPath)) {
      return { success: false, error: 'Transcription output not found' };
    }

    const jsonContent = fs.readFileSync(jsonOutputPath, 'utf-8');
    const whisperOutput: WhisperJsonOutput = JSON.parse(jsonContent);

    const segments = parseWhisperOutput(whisperOutput);

    fs.unlinkSync(wavPath);
    fs.unlinkSync(jsonOutputPath);

    onProgress?.(100);

    const subtitleData: SubtitleData = {
      segments,
      meta: {
        generatedAt: new Date().toISOString(),
        language: 'auto',
        model: options.model,
        prompt: options.prompt,
      },
    };

    return { success: true, data: subtitleData };
  } catch (error) {
    if (fs.existsSync(wavPath)) {
      fs.unlinkSync(wavPath);
    }
    if (fs.existsSync(jsonPath)) {
      fs.unlinkSync(jsonPath);
    }

    const errorMessage =
      error instanceof Error ? error.message : 'Unknown error';
    return { success: false, error: errorMessage };
  }
}
