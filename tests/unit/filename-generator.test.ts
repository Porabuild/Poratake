import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

describe('Filename Generator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    // Set a fixed date for consistent testing: 2025-03-15 14:30:45
    vi.setSystemTime(new Date(2025, 2, 15, 14, 30, 45));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('generateFilename', () => {
    it('should generate filename with default pattern', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%type %Y-%m-%d at %H.%M.%S',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('Screenshot 2025-03-15 at 14.30.45.png');
    });

    it('should replace %type token with Screenshot', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%type',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('Screenshot.png');
    });

    it('should replace %type token with Recording', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%type',
        type: 'Recording',
        extension: 'mov',
      });

      expect(filename).toBe('Recording.mov');
    });

    it('should replace %Y with full year', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%Y',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('2025.png');
    });

    it('should replace %m with zero-padded month', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%m',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('03.png');
    });

    it('should replace %d with zero-padded day', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%d',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('15.png');
    });

    it('should replace %H with zero-padded hour (24h)', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%H',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('14.png');
    });

    it('should replace %M with zero-padded minute', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%M',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('30.png');
    });

    it('should replace %S with zero-padded second', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%S',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('45.png');
    });

    it('should handle multiple occurrences of the same token', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%Y-%Y-%Y',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('2025-2025-2025.png');
    });

    it('should handle pattern with no tokens', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: 'my-screenshot',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('my-screenshot.png');
    });

    it('should work with different file extensions', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      expect(
        generateFilename({
          pattern: 'test',
          type: 'Screenshot',
          extension: 'jpg',
        })
      ).toBe('test.jpg');

      expect(
        generateFilename({
          pattern: 'test',
          type: 'Recording',
          extension: 'mp4',
        })
      ).toBe('test.mp4');

      expect(
        generateFilename({
          pattern: 'test',
          type: 'Recording',
          extension: 'gif',
        })
      ).toBe('test.gif');
    });

    it('should handle zero-padding for single digit values', async () => {
      // Set date to January 5th, 09:05:03
      vi.setSystemTime(new Date(2025, 0, 5, 9, 5, 3));

      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%m-%d %H:%M:%S',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('01-05 09-05-03.png');
    });
  });

  describe('sanitizeFilename', () => {
    it('should replace invalid characters with dashes', async () => {
      const { sanitizeFilename } =
        await import('@/main/utils/filename-generator');

      expect(sanitizeFilename('file<name')).toBe('file-name');
      expect(sanitizeFilename('file>name')).toBe('file-name');
      expect(sanitizeFilename('file:name')).toBe('file-name');
      expect(sanitizeFilename('file"name')).toBe('file-name');
      expect(sanitizeFilename('file/name')).toBe('file-name');
      expect(sanitizeFilename('file\\name')).toBe('file-name');
      expect(sanitizeFilename('file|name')).toBe('file-name');
      expect(sanitizeFilename('file?name')).toBe('file-name');
      expect(sanitizeFilename('file*name')).toBe('file-name');
    });

    it('should handle multiple invalid characters', async () => {
      const { sanitizeFilename } =
        await import('@/main/utils/filename-generator');

      expect(sanitizeFilename('file<>:name')).toBe('file---name');
    });

    it('should collapse multiple spaces into single space', async () => {
      const { sanitizeFilename } =
        await import('@/main/utils/filename-generator');

      expect(sanitizeFilename('file   name')).toBe('file name');
    });

    it('should trim leading and trailing whitespace', async () => {
      const { sanitizeFilename } =
        await import('@/main/utils/filename-generator');

      expect(sanitizeFilename('  filename  ')).toBe('filename');
    });

    it('should handle valid filenames unchanged', async () => {
      const { sanitizeFilename } =
        await import('@/main/utils/filename-generator');

      expect(sanitizeFilename('Screenshot 2025-03-15')).toBe(
        'Screenshot 2025-03-15'
      );
      expect(sanitizeFilename('my-file_name.test')).toBe('my-file_name.test');
    });
  });

  describe('validateNamingPattern', () => {
    it('should return null for valid patterns', async () => {
      const { validateNamingPattern } =
        await import('@/main/utils/filename-generator');

      expect(validateNamingPattern('%type %Y-%m-%d at %H.%M.%S')).toBeNull();
      expect(validateNamingPattern('Screenshot-%Y%m%d')).toBeNull();
      expect(validateNamingPattern('simple-name')).toBeNull();
    });

    it('should return error for empty pattern', async () => {
      const { validateNamingPattern } =
        await import('@/main/utils/filename-generator');

      expect(validateNamingPattern('')).toBe('Pattern cannot be empty');
      expect(validateNamingPattern('   ')).toBe('Pattern cannot be empty');
    });

    it('should return error for pattern over 100 characters', async () => {
      const { validateNamingPattern } =
        await import('@/main/utils/filename-generator');

      const longPattern = 'a'.repeat(101);
      expect(validateNamingPattern(longPattern)).toBe(
        'Pattern is too long (max 100 characters)'
      );
    });

    it('should allow pattern exactly 100 characters', async () => {
      const { validateNamingPattern } =
        await import('@/main/utils/filename-generator');

      const maxPattern = 'a'.repeat(100);
      expect(validateNamingPattern(maxPattern)).toBeNull();
    });

    it('should return error for patterns with invalid characters', async () => {
      const { validateNamingPattern } =
        await import('@/main/utils/filename-generator');

      expect(validateNamingPattern('file<name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file>name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file:name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file"name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file|name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file?name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file*name')).toBe(
        'Pattern contains invalid characters'
      );
    });

    it('should not flag token characters as invalid', async () => {
      const { validateNamingPattern } =
        await import('@/main/utils/filename-generator');

      // These contain % which is part of tokens, not invalid characters
      expect(validateNamingPattern('%Y-%m-%d')).toBeNull();
      expect(validateNamingPattern('%type')).toBeNull();
      expect(validateNamingPattern('%H%M%S')).toBeNull();
    });

    it('should validate consistently across multiple calls (no global regex state issue)', async () => {
      const { validateNamingPattern } =
        await import('@/main/utils/filename-generator');

      // This tests the fix for the global regex flag issue
      // Call validateNamingPattern multiple times with the same invalid input
      expect(validateNamingPattern('file<name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file<name')).toBe(
        'Pattern contains invalid characters'
      );
      expect(validateNamingPattern('file<name')).toBe(
        'Pattern contains invalid characters'
      );

      // And with valid input
      expect(validateNamingPattern('valid')).toBeNull();
      expect(validateNamingPattern('valid')).toBeNull();
    });
  });

  describe('getAvailableTokens', () => {
    it('should return all available tokens', async () => {
      const { getAvailableTokens } =
        await import('@/main/utils/filename-generator');

      const tokens = getAvailableTokens();

      expect(tokens).toHaveLength(7);
      expect(tokens.map(t => t.token)).toEqual([
        '%Y',
        '%m',
        '%d',
        '%H',
        '%M',
        '%S',
        '%type',
      ]);
    });

    it('should include descriptions for all tokens', async () => {
      const { getAvailableTokens } =
        await import('@/main/utils/filename-generator');

      const tokens = getAvailableTokens();

      tokens.forEach(token => {
        expect(token.description).toBeTruthy();
        expect(typeof token.description).toBe('string');
      });
    });

    it('should include valid examples for all tokens', async () => {
      const { getAvailableTokens } =
        await import('@/main/utils/filename-generator');

      const tokens = getAvailableTokens();

      // Check specific token examples based on our mocked time
      const yearToken = tokens.find(t => t.token === '%Y');
      expect(yearToken?.example).toBe('2025');

      const monthToken = tokens.find(t => t.token === '%m');
      expect(monthToken?.example).toBe('03');

      const dayToken = tokens.find(t => t.token === '%d');
      expect(dayToken?.example).toBe('15');

      const hourToken = tokens.find(t => t.token === '%H');
      expect(hourToken?.example).toBe('14');

      const minuteToken = tokens.find(t => t.token === '%M');
      expect(minuteToken?.example).toBe('30');

      const secondToken = tokens.find(t => t.token === '%S');
      expect(secondToken?.example).toBe('45');

      const typeToken = tokens.find(t => t.token === '%type');
      expect(typeToken?.example).toBe('Screenshot');
    });
  });

  describe('Edge cases', () => {
    it('should handle pattern with only spaces', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '   ',
        type: 'Screenshot',
        extension: 'png',
      });

      // After sanitization, should be empty then just have extension
      expect(filename).toBe('.png');
    });

    it('should handle CleanShot-style pattern', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      // CleanShot uses: CleanShot %Y-%m-%d at %H.%M.%S
      const filename = generateFilename({
        pattern: 'CleanShot %Y-%m-%d at %H.%M.%S',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('CleanShot 2025-03-15 at 14.30.45.png');
    });

    it('should handle pattern with consecutive tokens', async () => {
      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%Y%m%d%H%M%S',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('20250315143045.png');
    });

    it('should handle midnight (00:00:00)', async () => {
      vi.setSystemTime(new Date(2025, 0, 1, 0, 0, 0));

      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%H:%M:%S',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('00-00-00.png');
    });

    it('should handle end of year (December 31st)', async () => {
      vi.setSystemTime(new Date(2025, 11, 31, 23, 59, 59));

      const { generateFilename } =
        await import('@/main/utils/filename-generator');

      const filename = generateFilename({
        pattern: '%Y-%m-%d %H:%M:%S',
        type: 'Screenshot',
        extension: 'png',
      });

      expect(filename).toBe('2025-12-31 23-59-59.png');
    });
  });
});
