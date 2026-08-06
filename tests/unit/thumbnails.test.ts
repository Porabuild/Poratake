import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockExistsSync = vi.fn();
const mockMkdirSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockRenameSync = vi.fn();
const mockRmSync = vi.fn();

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
    readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
    renameSync: (...a: unknown[]) => mockRenameSync(...a),
    rmSync: (...a: unknown[]) => mockRmSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  renameSync: (...a: unknown[]) => mockRenameSync(...a),
  rmSync: (...a: unknown[]) => mockRmSync(...a),
}));

const mockExecFile = vi.fn();
vi.mock('child_process', () => ({
  execFile: (
    cmd: string,
    args: string[],
    cb: (err: Error | null, out?: { stdout: string; stderr: string }) => void
  ) => mockExecFile(cmd, args, cb),
}));

vi.mock('@/main/utils/paths', () => ({
  getConfigDir: () => '/mock/config',
}));

const mockGenerateVideoThumbnail = vi.fn();
vi.mock('@/main/utils/ffmpeg', () => ({
  generateVideoThumbnail: (...a: unknown[]) => mockGenerateVideoThumbnail(...a),
}));

describe('thumbnails', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('getThumbnail', () => {
    it('returns null when original file is missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/missing.png', 'screenshot');
      expect(result).toEqual({ base64: null, cached: false });
    });

    it('returns cached thumbnail base64 when present', async () => {
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(Buffer.from('binary'));
      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/photo.png', 'screenshot');
      expect(result.cached).toBe(true);
      expect(result.base64).toBe(Buffer.from('binary').toString('base64'));
    });

    it('generates image thumbnail with sips when not cached', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 1) return true;
        if (existsCallCount === 2) return true;
        if (existsCallCount === 3) return false;
        return true;
      });
      mockReadFileSync.mockReturnValue(Buffer.from('thumb'));
      mockExecFile.mockImplementation((_c, _a, cb) =>
        cb(null, { stdout: '', stderr: '' })
      );

      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/photo.jpg', 'screenshot');
      expect(mockExecFile).toHaveBeenCalled();
      expect(result.base64).toBe(Buffer.from('thumb').toString('base64'));
      expect(result.cached).toBe(false);
    });

    it('generates video thumbnail via ffmpeg when not cached', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 1) return true;
        if (existsCallCount === 2) return true;
        if (existsCallCount === 3) return false;
        return true;
      });
      mockReadFileSync.mockReturnValue(Buffer.from('thumb'));
      mockGenerateVideoThumbnail.mockResolvedValue({ success: true });
      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/clip.mov', 'video');
      expect(mockGenerateVideoThumbnail).toHaveBeenCalled();
      expect(result.base64).toBe(Buffer.from('thumb').toString('base64'));
    });

    it('returns null when generation fails', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 1) return true;
        if (existsCallCount === 2) return true;
        if (existsCallCount === 3) return false;
        return false;
      });
      mockGenerateVideoThumbnail.mockResolvedValue({ success: false });
      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/clip.mov', 'video');
      expect(result.base64).toBeNull();
    });

    it('makes thumbnails directory if missing', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 1) return true;
        if (existsCallCount === 2) return false;
        if (existsCallCount === 3) return false;
        return false;
      });
      mockGenerateVideoThumbnail.mockResolvedValue({ success: false });
      const { getThumbnail } = await import('@/main/utils/thumbnails');
      await getThumbnail('/path/clip.mov', 'video');
      expect(mockMkdirSync).toHaveBeenCalled();
    });
  });

  describe('rekeyThumbnail', () => {
    it('renames when old thumbnail exists', async () => {
      mockExistsSync.mockReturnValue(true);
      const { rekeyThumbnail } = await import('@/main/utils/thumbnails');
      rekeyThumbnail('/old.png', '/new.png');
      expect(mockRenameSync).toHaveBeenCalled();
    });

    it('does not rename when old thumbnail is missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { rekeyThumbnail } = await import('@/main/utils/thumbnails');
      rekeyThumbnail('/old.png', '/new.png');
      expect(mockRenameSync).not.toHaveBeenCalled();
    });

    it('swallows rename errors', async () => {
      mockExistsSync.mockReturnValue(true);
      mockRenameSync.mockImplementation(() => {
        throw new Error('busy');
      });
      const { rekeyThumbnail } = await import('@/main/utils/thumbnails');
      expect(() => rekeyThumbnail('/old.png', '/new.png')).not.toThrow();
    });
  });

  describe('deleteThumbnail', () => {
    it('unlinks when exists', async () => {
      mockExistsSync.mockReturnValue(true);
      const { deleteThumbnail } = await import('@/main/utils/thumbnails');
      deleteThumbnail('/p.png');
      expect(mockUnlinkSync).toHaveBeenCalled();
    });

    it('skips when missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { deleteThumbnail } = await import('@/main/utils/thumbnails');
      deleteThumbnail('/p.png');
      expect(mockUnlinkSync).not.toHaveBeenCalled();
    });

    it('swallows unlink errors', async () => {
      mockExistsSync.mockReturnValue(true);
      mockUnlinkSync.mockImplementation(() => {
        throw new Error('locked');
      });
      const { deleteThumbnail } = await import('@/main/utils/thumbnails');
      expect(() => deleteThumbnail('/p.png')).not.toThrow();
    });
  });

  describe('clearAllThumbnails', () => {
    it('removes thumbnails directory when present', async () => {
      mockExistsSync.mockReturnValue(true);
      const { clearAllThumbnails } = await import('@/main/utils/thumbnails');
      clearAllThumbnails();
      expect(mockRmSync).toHaveBeenCalled();
    });

    it('skips when directory absent', async () => {
      mockExistsSync.mockReturnValue(false);
      const { clearAllThumbnails } = await import('@/main/utils/thumbnails');
      clearAllThumbnails();
      expect(mockRmSync).not.toHaveBeenCalled();
    });

    it('swallows rm errors', async () => {
      mockExistsSync.mockReturnValue(true);
      mockRmSync.mockImplementation(() => {
        throw new Error('busy');
      });
      const { clearAllThumbnails } = await import('@/main/utils/thumbnails');
      expect(() => clearAllThumbnails()).not.toThrow();
    });
  });
});
