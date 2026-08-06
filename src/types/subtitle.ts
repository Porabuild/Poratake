export interface SubtitleSegment {
  start: number;
  end: number;
  text: string;
  words?: SubtitleWord[];
}

export interface SubtitleWord {
  text: string;
  start: number;
  end: number;
}

export interface SubtitleData {
  segments: SubtitleSegment[];
  meta: {
    generatedAt: string;
    language: string;
    model: SubtitleSource;
    prompt?: string;
  };
}

export type WhisperModel = 'base' | 'small' | 'medium';

export type SubtitleSource = WhisperModel | 'manual' | 'imported';

export interface SubtitleStyle {
  visible: boolean;
  fontSize: 'small' | 'medium' | 'large';
  position: 'bottom' | 'top';
  backgroundColor: 'dark' | 'light' | 'none';
  opacity: number;
}

export const DEFAULT_SUBTITLE_STYLE: SubtitleStyle = {
  visible: true,
  fontSize: 'medium',
  position: 'bottom',
  backgroundColor: 'dark',
  opacity: 0.9,
};

export const WHISPER_MODELS: {
  id: WhisperModel;
  label: string;
  downloadSize: string;
  memoryUsage: string;
  description: string;
}[] = [
  {
    id: 'base',
    label: 'Base',
    downloadSize: '~142 MB',
    memoryUsage: '~500 MB',
    description: 'Fast, basic accuracy',
  },
  {
    id: 'small',
    label: 'Small',
    downloadSize: '~466 MB',
    memoryUsage: '~1 GB',
    description: 'Balanced speed and accuracy',
  },
  {
    id: 'medium',
    label: 'Medium',
    downloadSize: '~1.5 GB',
    memoryUsage: '~2.6 GB',
    description: 'Slower, higher accuracy',
  },
];

export interface SubtitleGenerationOptions {
  model: WhisperModel;
  prompt?: string;
}

export type WhisperDownloadStatus =
  | { status: 'not-checked' }
  | { status: 'checking' }
  | { status: 'not-downloaded' }
  | { status: 'downloading'; progress: number; item: 'binary' | WhisperModel }
  | { status: 'ready' };

export type SubtitleGenerationStatus =
  | { status: 'idle' }
  | { status: 'generating'; progress: number }
  | { status: 'complete' }
  | { status: 'error'; message: string };

export interface SubtitleDataValidationResult {
  valid: boolean;
  error?: string;
  data?: SubtitleData;
}

export function validateSubtitleData(
  data: unknown
): SubtitleDataValidationResult {
  if (!data || typeof data !== 'object') {
    return { valid: false, error: 'Invalid data: expected an object' };
  }

  const obj = data as Record<string, unknown>;

  if (!Array.isArray(obj.segments)) {
    return { valid: false, error: 'Invalid segments: expected an array' };
  }

  for (let i = 0; i < obj.segments.length; i++) {
    const segment = obj.segments[i];
    if (!segment || typeof segment !== 'object') {
      return { valid: false, error: `Invalid segment at index ${i}` };
    }

    const s = segment as Record<string, unknown>;
    if (typeof s.start !== 'number' || s.start < 0) {
      return {
        valid: false,
        error: `Invalid start at segment ${i}: expected non-negative number`,
      };
    }
    if (typeof s.end !== 'number' || s.end < 0) {
      return {
        valid: false,
        error: `Invalid end at segment ${i}: expected non-negative number`,
      };
    }
    if (s.end <= s.start) {
      return {
        valid: false,
        error: `Invalid timing at segment ${i}: end must be greater than start`,
      };
    }
    if (typeof s.text !== 'string') {
      return {
        valid: false,
        error: `Invalid text at segment ${i}: expected string`,
      };
    }

    if (s.words !== undefined) {
      if (!Array.isArray(s.words)) {
        return {
          valid: false,
          error: `Invalid words at segment ${i}: expected array`,
        };
      }
      for (let j = 0; j < s.words.length; j++) {
        const word = s.words[j];
        if (!word || typeof word !== 'object') {
          return {
            valid: false,
            error: `Invalid word at segment ${i}, word ${j}`,
          };
        }
        const w = word as Record<string, unknown>;
        if (typeof w.text !== 'string') {
          return {
            valid: false,
            error: `Invalid word text at segment ${i}, word ${j}`,
          };
        }
        if (typeof w.start !== 'number' || typeof w.end !== 'number') {
          return {
            valid: false,
            error: `Invalid word timing at segment ${i}, word ${j}`,
          };
        }
      }
    }
  }

  if (!obj.meta || typeof obj.meta !== 'object') {
    return { valid: false, error: 'Invalid meta: expected an object' };
  }

  const meta = obj.meta as Record<string, unknown>;
  if (typeof meta.generatedAt !== 'string') {
    return { valid: false, error: 'Invalid meta.generatedAt: expected string' };
  }
  if (typeof meta.language !== 'string') {
    return { valid: false, error: 'Invalid meta.language: expected string' };
  }
  if (typeof meta.model !== 'string') {
    return { valid: false, error: 'Invalid meta.model: expected string' };
  }

  return { valid: true, data: data as SubtitleData };
}
