export type CaptureType = 'Screenshot' | 'Recording';

export interface FilenameOptions {
  pattern: string;
  type: CaptureType;
  extension: string;
}

export interface TokenInfo {
  token: string;
  description: string;
  example: string;
}

const TOKEN_FORMATTERS: Record<string, (date: Date) => string> = {
  '%Y': date => date.getFullYear().toString(),
  '%m': date => (date.getMonth() + 1).toString().padStart(2, '0'),
  '%d': date => date.getDate().toString().padStart(2, '0'),
  '%H': date => date.getHours().toString().padStart(2, '0'),
  '%M': date => date.getMinutes().toString().padStart(2, '0'),
  '%S': date => date.getSeconds().toString().padStart(2, '0'),
};

const INVALID_FILENAME_CHARS = /[<>:"/\\|?*]/g;
const INVALID_FILENAME_CHARS_TEST = /[<>:"/\\|?*]/;

export function generateFilename(options: FilenameOptions): string {
  const { pattern, type, extension } = options;
  const date = new Date();

  let filename = pattern;

  filename = filename.replace(/%type/g, type);

  for (const [token, formatter] of Object.entries(TOKEN_FORMATTERS)) {
    filename = filename.replace(new RegExp(token, 'g'), formatter(date));
  }

  filename = sanitizeFilename(filename);

  return `${filename}.${extension}`;
}

export function sanitizeFilename(filename: string): string {
  return filename
    .replace(INVALID_FILENAME_CHARS, '-')
    .replace(/\s+/g, ' ')
    .trim();
}

export function validateNamingPattern(pattern: string): string | null {
  if (!pattern || pattern.trim().length === 0) {
    return 'Pattern cannot be empty';
  }

  if (pattern.length > 100) {
    return 'Pattern is too long (max 100 characters)';
  }

  const withoutTokens = pattern.replace(/%[YmdHMS]|%type/g, '');
  if (INVALID_FILENAME_CHARS_TEST.test(withoutTokens)) {
    return 'Pattern contains invalid characters';
  }

  return null;
}

export function getAvailableTokens(): TokenInfo[] {
  const now = new Date();
  return [
    {
      token: '%Y',
      description: 'Full year',
      example: TOKEN_FORMATTERS['%Y'](now),
    },
    {
      token: '%m',
      description: 'Month (01-12)',
      example: TOKEN_FORMATTERS['%m'](now),
    },
    {
      token: '%d',
      description: 'Day (01-31)',
      example: TOKEN_FORMATTERS['%d'](now),
    },
    {
      token: '%H',
      description: 'Hour (00-23)',
      example: TOKEN_FORMATTERS['%H'](now),
    },
    {
      token: '%M',
      description: 'Minute (00-59)',
      example: TOKEN_FORMATTERS['%M'](now),
    },
    {
      token: '%S',
      description: 'Second (00-59)',
      example: TOKEN_FORMATTERS['%S'](now),
    },
    { token: '%type', description: 'Capture type', example: 'Screenshot' },
  ];
}
