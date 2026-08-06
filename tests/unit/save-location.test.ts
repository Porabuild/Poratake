import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockIsExistingDirectory = vi.fn();

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/utils/paths', () => ({
  isExistingDirectory: (...a: unknown[]) => mockIsExistingDirectory(...a),
}));

async function importModule() {
  return import('@/main/utils/save-location');
}

describe('save location', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockIsExistingDirectory.mockReturnValue(true);
  });

  describe('getLastSaveDirectory', () => {
    it('returns the remembered screenshot directory', async () => {
      mockGetConfig.mockReturnValue({
        saveLocations: { screenshot: '/Users/me/Desktop' },
      });
      const { getLastSaveDirectory } = await importModule();
      expect(getLastSaveDirectory('screenshot')).toBe('/Users/me/Desktop');
    });

    it('returns the remembered video directory', async () => {
      mockGetConfig.mockReturnValue({
        saveLocations: { video: '/Users/me/Movies' },
      });
      const { getLastSaveDirectory } = await importModule();
      expect(getLastSaveDirectory('video')).toBe('/Users/me/Movies');
    });

    it('keeps the kinds isolated from each other', async () => {
      mockGetConfig.mockReturnValue({
        saveLocations: { screenshot: '/Users/me/Desktop' },
      });
      const { getLastSaveDirectory } = await importModule();
      expect(getLastSaveDirectory('video')).toBeNull();
    });

    it('returns null when nothing was remembered', async () => {
      mockGetConfig.mockReturnValue({ saveLocations: {} });
      const { getLastSaveDirectory } = await importModule();
      expect(getLastSaveDirectory('screenshot')).toBeNull();
    });

    it('returns null when the section is missing', async () => {
      mockGetConfig.mockReturnValue({});
      const { getLastSaveDirectory } = await importModule();
      expect(getLastSaveDirectory('screenshot')).toBeNull();
    });

    it('returns null when the directory no longer exists', async () => {
      mockGetConfig.mockReturnValue({
        saveLocations: { screenshot: '/gone' },
      });
      mockIsExistingDirectory.mockReturnValue(false);
      const { getLastSaveDirectory } = await importModule();
      expect(getLastSaveDirectory('screenshot')).toBeNull();
    });
  });

  describe('resolveSaveDialogPath', () => {
    it('joins the remembered directory with the file name', async () => {
      mockGetConfig.mockReturnValue({
        saveLocations: { screenshot: '/Users/me/Desktop' },
      });
      const { resolveSaveDialogPath } = await importModule();
      expect(resolveSaveDialogPath('screenshot', 'Shot.png', '/Pictures')).toBe(
        '/Users/me/Desktop/Shot.png'
      );
    });

    it('falls back to the provided directory', async () => {
      mockGetConfig.mockReturnValue({ saveLocations: {} });
      const { resolveSaveDialogPath } = await importModule();
      expect(resolveSaveDialogPath('screenshot', 'Shot.png', '/Pictures')).toBe(
        '/Pictures/Shot.png'
      );
    });

    it('returns the bare file name without a fallback directory', async () => {
      mockGetConfig.mockReturnValue({ saveLocations: {} });
      const { resolveSaveDialogPath } = await importModule();
      expect(resolveSaveDialogPath('video', 'Clip.mp4')).toBe('Clip.mp4');
    });
  });

  describe('rememberSaveDirectory', () => {
    it('persists the directory of the chosen file', async () => {
      mockGetConfig.mockReturnValue({
        saveLocations: { screenshot: '', video: '/Users/me/Movies' },
      });
      const { rememberSaveDirectory } = await importModule();
      rememberSaveDirectory('screenshot', '/Users/me/Desktop/Shot.png');
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        saveLocations: {
          screenshot: '/Users/me/Desktop',
          video: '/Users/me/Movies',
        },
      });
    });

    it('persists video exports under their own key', async () => {
      mockGetConfig.mockReturnValue({ saveLocations: {} });
      const { rememberSaveDirectory } = await importModule();
      rememberSaveDirectory('video', '/Users/me/Movies/Clip.mp4');
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        saveLocations: { screenshot: '', video: '/Users/me/Movies' },
      });
    });

    it('skips the write when the directory is unchanged', async () => {
      mockGetConfig.mockReturnValue({
        saveLocations: { screenshot: '/Users/me/Desktop' },
      });
      const { rememberSaveDirectory } = await importModule();
      rememberSaveDirectory('screenshot', '/Users/me/Desktop/Shot.png');
      expect(mockUpdateConfig).not.toHaveBeenCalled();
    });

    it('fills defaults when the section is missing', async () => {
      mockGetConfig.mockReturnValue({});
      const { rememberSaveDirectory } = await importModule();
      rememberSaveDirectory('screenshot', '/Users/me/Desktop/Shot.png');
      expect(mockUpdateConfig).toHaveBeenCalledWith({
        saveLocations: { screenshot: '/Users/me/Desktop', video: '' },
      });
    });
  });
});
