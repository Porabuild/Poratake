import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockGetConfig = vi.fn();
const mockGenerateFilename = vi.fn();
const mockEnsureDirectoryExists = vi.fn();
const mockIsValidDirectory = vi.fn();
const mockGetPath = vi.fn();

vi.mock('electron', () => ({
  app: { getPath: (...a: unknown[]) => mockGetPath(...a) },
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/utils/filename-generator', () => ({
  generateFilename: (...a: unknown[]) => mockGenerateFilename(...a),
}));

vi.mock('@/main/utils/paths', () => ({
  ensureDirectoryExists: (...a: unknown[]) => mockEnsureDirectoryExists(...a),
  isValidDirectory: (...a: unknown[]) => mockIsValidDirectory(...a),
}));

describe('screenshot utils', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockEnsureDirectoryExists.mockImplementation((p: string) => p);
    mockGenerateFilename.mockReturnValue('Screenshot.png');
    mockGetPath.mockReturnValue('/Users/me/Pictures');
  });

  describe('getScreenshotsDir', () => {
    it('uses custom path when valid', async () => {
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: '/custom/dir' },
      });
      mockIsValidDirectory.mockReturnValue(true);
      const { getScreenshotsDir } =
        await import('@/main/capture/screenshot/utils');
      expect(getScreenshotsDir()).toBe('/custom/dir');
    });

    it('falls back to Pictures/Capty when custom path invalid', async () => {
      mockGetConfig.mockReturnValue({
        storage: { screenshotsPath: '/bad/dir' },
      });
      mockIsValidDirectory.mockReturnValue(false);
      const { getScreenshotsDir } =
        await import('@/main/capture/screenshot/utils');
      expect(getScreenshotsDir()).toBe('/Users/me/Pictures/Capty');
    });

    it('uses default Pictures/Capty when no custom path', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      const { getScreenshotsDir } =
        await import('@/main/capture/screenshot/utils');
      expect(getScreenshotsDir()).toBe('/Users/me/Pictures/Capty');
    });
  });

  describe('generateScreenshotPath', () => {
    it('combines directory and generated filename', async () => {
      mockGetConfig.mockReturnValue({
        storage: { namingPattern: 'CustomPattern' },
      });
      mockIsValidDirectory.mockReturnValue(false);
      mockGenerateFilename.mockReturnValue('My Screenshot.png');
      const { generateScreenshotPath } =
        await import('@/main/capture/screenshot/utils');
      const result = generateScreenshotPath();
      expect(result).toBe('/Users/me/Pictures/Capty/My Screenshot.png');
      expect(mockGenerateFilename).toHaveBeenCalledWith({
        pattern: 'CustomPattern',
        type: 'Screenshot',
        extension: 'png',
      });
    });

    it('uses default pattern when not configured', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      const { generateScreenshotPath } =
        await import('@/main/capture/screenshot/utils');
      generateScreenshotPath();
      const [args] = mockGenerateFilename.mock.calls[0];
      expect(args.pattern).toBeDefined();
    });
  });

  describe('generateScreenshotExportName', () => {
    it('uses provided extension', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      mockGenerateFilename.mockReturnValue('Screenshot.jpg');
      const { generateScreenshotExportName } =
        await import('@/main/capture/screenshot/utils');
      const name = generateScreenshotExportName('jpg');
      expect(name).toBe('Screenshot.jpg');
      const [args] = mockGenerateFilename.mock.calls[0];
      expect(args.extension).toBe('jpg');
    });

    it('defaults extension to png', async () => {
      mockGetConfig.mockReturnValue({ storage: {} });
      const { generateScreenshotExportName } =
        await import('@/main/capture/screenshot/utils');
      generateScreenshotExportName();
      const [args] = mockGenerateFilename.mock.calls[0];
      expect(args.extension).toBe('png');
    });
  });
});
