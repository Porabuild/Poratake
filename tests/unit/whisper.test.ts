import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';

const mockExistsSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockRenameSync = vi.fn();
const mockCreateWriteStream = vi.fn();

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
    renameSync: (...a: unknown[]) => mockRenameSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  renameSync: (...a: unknown[]) => mockRenameSync(...a),
  createWriteStream: (...a: unknown[]) => mockCreateWriteStream(...a),
}));

const mockEnsureDirectory = vi.fn();
vi.mock('@/main/utils/paths', () => ({
  getConfigDir: () => '/mock/config',
  getNativeBinaryPath: (name: string) => `/mock/bin/${name}`,
  ensureDirectoryExists: (p: string) => mockEnsureDirectory(p),
}));

describe('whisper utilities', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('path helpers', () => {
    it('getWhisperDir composes config dir', async () => {
      const { getWhisperDir } = await import('@/main/utils/whisper');
      expect(getWhisperDir()).toBe(path.join('/mock/config', 'whisper'));
    });

    it('getWhisperCliPath returns binary path', async () => {
      const { getWhisperCliPath } = await import('@/main/utils/whisper');
      expect(getWhisperCliPath()).toBe('/mock/bin/whisper');
    });

    it('getWhisperModelPath returns model path for each model', async () => {
      const { getWhisperModelPath } = await import('@/main/utils/whisper');
      expect(getWhisperModelPath('base')).toBe(
        path.join('/mock/config', 'whisper', 'ggml-base.bin')
      );
      expect(getWhisperModelPath('small')).toBe(
        path.join('/mock/config', 'whisper', 'ggml-small.bin')
      );
      expect(getWhisperModelPath('medium')).toBe(
        path.join('/mock/config', 'whisper', 'ggml-medium.bin')
      );
    });
  });

  describe('availability', () => {
    it('isWhisperBinaryAvailable returns existsSync result', async () => {
      mockExistsSync.mockReturnValue(true);
      const { isWhisperBinaryAvailable } = await import('@/main/utils/whisper');
      expect(isWhisperBinaryAvailable()).toBe(true);
      mockExistsSync.mockReturnValue(false);
      expect(isWhisperBinaryAvailable()).toBe(false);
    });

    it('isWhisperModelAvailable checks specific model file', async () => {
      mockExistsSync.mockReturnValue(true);
      const { isWhisperModelAvailable } = await import('@/main/utils/whisper');
      expect(isWhisperModelAvailable('base')).toBe(true);
      expect(mockExistsSync).toHaveBeenCalledWith(
        path.join('/mock/config', 'whisper', 'ggml-base.bin')
      );
    });

    it('isWhisperReady combines binary + model', async () => {
      mockExistsSync.mockReturnValue(true);
      const { isWhisperReady } = await import('@/main/utils/whisper');
      expect(isWhisperReady('small')).toBe(true);
      mockExistsSync.mockReturnValue(false);
      expect(isWhisperReady('small')).toBe(false);
    });

    it('getAvailableModels returns models present', async () => {
      mockExistsSync.mockImplementation((p: unknown) => {
        const filePath = String(p);
        return (
          filePath.includes('ggml-base') || filePath.includes('ggml-medium')
        );
      });
      const { getAvailableModels } = await import('@/main/utils/whisper');
      expect(getAvailableModels()).toEqual(['base', 'medium']);
    });
  });

  describe('ensureWhisperReady', () => {
    it('throws when binary missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { ensureWhisperReady } = await import('@/main/utils/whisper');
      await expect(ensureWhisperReady('base')).rejects.toThrow(
        'Whisper binary not found'
      );
    });

    it('returns early when binary and model are present', async () => {
      mockExistsSync.mockReturnValue(true);
      const { ensureWhisperReady } = await import('@/main/utils/whisper');
      await expect(ensureWhisperReady('base')).resolves.toBeUndefined();
      expect(mockEnsureDirectory).not.toHaveBeenCalled();
    });
  });
});
