import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';

const mockExistsSync = vi.fn();
const mockMkdir = vi.fn();
const mockReadFile = vi.fn();
const mockWriteFile = vi.fn();
const mockUnlinkSync = vi.fn();
const mockRenameSync = vi.fn();
const mockRmSync = vi.fn();

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    promises: {
      mkdir: (...a: unknown[]) => mockMkdir(...a),
      readFile: (...a: unknown[]) => mockReadFile(...a),
      writeFile: (...a: unknown[]) => mockWriteFile(...a),
    },
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
    renameSync: (...a: unknown[]) => mockRenameSync(...a),
    rmSync: (...a: unknown[]) => mockRmSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  renameSync: (...a: unknown[]) => mockRenameSync(...a),
  rmSync: (...a: unknown[]) => mockRmSync(...a),
}));

const mockResize = vi.fn();
const mockToJPEG = vi.fn();
const mockIsEmpty = vi.fn();
const mockGetSize = vi.fn();
const mockCreateThumbnailFromPath = vi.fn();

vi.mock('electron', () => ({
  nativeImage: {
    createThumbnailFromPath: (...a: unknown[]) =>
      mockCreateThumbnailFromPath(...a),
  },
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

    mockMkdir.mockResolvedValue(undefined);
    mockWriteFile.mockResolvedValue(undefined);
    mockIsEmpty.mockReturnValue(false);
    mockGetSize.mockReturnValue({ width: 1200, height: 800 });
    mockToJPEG.mockReturnValue(Buffer.from('jpeg'));
    mockResize.mockImplementation(() => ({ toJPEG: mockToJPEG }));
    mockCreateThumbnailFromPath.mockResolvedValue({
      isEmpty: mockIsEmpty,
      getSize: mockGetSize,
      resize: mockResize,
      toJPEG: mockToJPEG,
    });
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
      mockReadFile.mockResolvedValue(Buffer.from('binary'));
      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/photo.png', 'screenshot');
      expect(result.cached).toBe(true);
      expect(result.base64).toBe(Buffer.from('binary').toString('base64'));
    });

    it('generates image thumbnail with nativeImage when not cached', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 1) return true;
        if (existsCallCount === 2) return false;
        return true;
      });
      mockReadFile.mockResolvedValue(Buffer.from('thumb'));

      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/photo.jpg', 'screenshot');
      expect(mockCreateThumbnailFromPath).toHaveBeenCalledWith(
        '/path/photo.jpg',
        { width: 300, height: 300 }
      );
      expect(mockResize).toHaveBeenCalledWith({ width: 300, quality: 'good' });
      expect(mockToJPEG).toHaveBeenCalledWith(80);
      expect(mockWriteFile).toHaveBeenCalled();
      expect(result.base64).toBe(Buffer.from('thumb').toString('base64'));
      expect(result.cached).toBe(false);
    });

    it('skips upscaling when the image is narrower than the thumbnail width', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 2) return false;
        return true;
      });
      mockGetSize.mockReturnValue({ width: 120, height: 80 });
      mockReadFile.mockResolvedValue(Buffer.from('thumb'));

      const { getThumbnail } = await import('@/main/utils/thumbnails');
      await getThumbnail('/path/small.png', 'screenshot');
      expect(mockResize).not.toHaveBeenCalled();
      expect(mockToJPEG).toHaveBeenCalledWith(80);
    });

    it('returns null when the image cannot be decoded', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 1) return true;
        return false;
      });
      mockIsEmpty.mockReturnValue(true);

      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const result = await getThumbnail('/path/broken.png', 'screenshot');
      expect(mockWriteFile).not.toHaveBeenCalled();
      expect(result.base64).toBeNull();
    });

    it('generates video thumbnail via ffmpeg when not cached', async () => {
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        if (existsCallCount === 1) return true;
        if (existsCallCount === 2) return false;
        return true;
      });
      mockReadFile.mockResolvedValue(Buffer.from('thumb'));
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
      expect(mockMkdir).toHaveBeenCalledWith(
        path.join('/mock/config', 'thumbnails'),
        { recursive: true }
      );
    });

    it('shares thumbnail generation between concurrent requests', async () => {
      let finishGeneration: (value: { success: boolean }) => void = () => {};
      let existsCallCount = 0;
      mockExistsSync.mockImplementation(() => {
        existsCallCount++;
        return existsCallCount !== 2;
      });
      mockGenerateVideoThumbnail.mockReturnValueOnce(
        new Promise(resolve => {
          finishGeneration = resolve;
        })
      );
      mockReadFile.mockResolvedValue(Buffer.from('thumb'));

      const { getThumbnail } = await import('@/main/utils/thumbnails');
      const first = getThumbnail('/path/clip.mov', 'video');
      const second = getThumbnail('/path/clip.mov', 'video');

      await vi.waitFor(() => {
        expect(mockGenerateVideoThumbnail).toHaveBeenCalledTimes(1);
      });
      finishGeneration({ success: true });

      await expect(Promise.all([first, second])).resolves.toEqual([
        {
          base64: Buffer.from('thumb').toString('base64'),
          cached: false,
        },
        {
          base64: Buffer.from('thumb').toString('base64'),
          cached: false,
        },
      ]);
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
