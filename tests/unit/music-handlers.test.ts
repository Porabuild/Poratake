import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockExecFileAsync = vi.fn();
const mockGetWindowData = vi.fn();
const mockShowOpenDialog = vi.fn();
const mockExistsSync = vi.fn();
const mockMkdir = vi.fn();
const mockCopyFile = vi.fn();
const mockUnlink = vi.fn();
const mockGetExportAbortSignal = vi.fn();
const exportEvent = { sender: { id: 1 } };

vi.mock('electron', () => ({
  ipcMain: {
    handle: (e: string, h: Handler) => {
      ipcHandle[e] = h;
    },
  },
  dialog: { showOpenDialog: (...a: unknown[]) => mockShowOpenDialog(...a) },
  BrowserWindow: { fromWebContents: vi.fn(() => ({ id: 1 })) },
}));

vi.mock('fs', () => ({
  default: { existsSync: (...a: unknown[]) => mockExistsSync(...a) },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
}));

vi.mock('fs/promises', () => ({
  default: {
    mkdir: (...a: unknown[]) => mockMkdir(...a),
    copyFile: (...a: unknown[]) => mockCopyFile(...a),
    unlink: (...a: unknown[]) => mockUnlink(...a),
  },
  mkdir: (...a: unknown[]) => mockMkdir(...a),
  copyFile: (...a: unknown[]) => mockCopyFile(...a),
  unlink: (...a: unknown[]) => mockUnlink(...a),
}));

vi.mock('child_process', () => ({
  execFile: vi.fn(),
}));

vi.mock('util', () => ({
  promisify:
    () =>
    async (...args: unknown[]) =>
      mockExecFileAsync(...args),
}));

vi.mock('@/main/capture/video/window-manager', () => ({
  getWindowData: (...a: unknown[]) => mockGetWindowData(...a),
}));

vi.mock('@/main/capture/video/recording-project', () => ({
  getMusicFolderPath: (p: string) => {
    if (!p.includes('.poratake')) return null;
    if (p.endsWith('.poratake')) return `${p}/music`;
    const idx = p.indexOf('.poratake');
    return `${p.slice(0, idx + '.poratake'.length)}/music`;
  },
}));

vi.mock('@/main/utils/ffmpeg', () => ({
  getFFmpegPath: () => '/bin/ffmpeg',
}));

vi.mock('@/main/capture/video/ipc/export-session', () => ({
  getExportAbortSignal: (...args: unknown[]) =>
    mockGetExportAbortSignal(...args),
  isExportOutputPathAllowed: () => true,
}));

describe('music handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockGetExportAbortSignal.mockReturnValue(undefined);
  });

  describe('music:add', () => {
    it('returns error without window data', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = await ipcHandle['video-editor:music:add']({
        sender: { id: 1 },
      });
      expect((result as { success: boolean }).success).toBe(false);
    });

    it('returns error when user cancels dialog', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockShowOpenDialog.mockResolvedValue({
        canceled: true,
        filePaths: [],
      });
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:add']({
        sender: { id: 1 },
      })) as { success: boolean; error?: string };
      expect(result.success).toBe(false);
      expect(result.error).toBe('Cancelled');
    });

    it('returns error when music folder cannot be resolved', async () => {
      mockGetWindowData.mockReturnValue({ filePath: '/p/legacy.mov' });
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/path/to/song.mp3'],
      });
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:add']({
        sender: { id: 1 },
      })) as { success: boolean; error?: string };
      expect(result.success).toBe(false);
      expect(result.error).toBe('Could not resolve music folder');
    });

    it('copies file and probes duration', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/path/song.mp3'],
      });
      mockExistsSync.mockReturnValue(false);
      mockMkdir.mockResolvedValue(undefined);
      mockCopyFile.mockResolvedValue(undefined);
      mockExecFileAsync.mockImplementation(() => {
        const error = new Error('ffmpeg always errors on info');
        (error as { stderr?: string }).stderr =
          'Duration: 00:01:23.45, start: 0';
        throw error;
      });

      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:add']({
        sender: { id: 1 },
      })) as {
        success: boolean;
        fileName?: string;
        originalDuration?: number;
      };
      expect(result.success).toBe(true);
      expect(result.fileName).toBe('song.mp3');
      expect(result.originalDuration).toBeCloseTo(83.45, 1);
    });

    it('returns error when copyFile fails', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/path/song.mp3'],
      });
      let calls = 0;
      mockExistsSync.mockImplementation(() => {
        calls++;
        return calls === 1;
      });
      mockCopyFile.mockRejectedValue(new Error('disk full'));
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:add']({
        sender: { id: 1 },
      })) as { success: boolean; error?: string };
      expect(result.success).toBe(false);
      expect(result.error).toBe('disk full');
    });

    it('returns error when duration is 0', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/path/song.mp3'],
      });
      let calls = 0;
      mockExistsSync.mockImplementation(() => {
        calls++;
        return calls === 1;
      });
      mockCopyFile.mockResolvedValue(undefined);
      mockExecFileAsync.mockImplementation(() => {
        const error = new Error('boom');
        (error as { stderr?: string }).stderr = 'No duration info';
        throw error;
      });
      mockUnlink.mockResolvedValue(undefined);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:add']({
        sender: { id: 1 },
      })) as { success: boolean; error?: string };
      expect(result.success).toBe(false);
      expect(result.error).toBe('Could not determine audio duration');
    });
  });

  describe('music:remove', () => {
    it('returns error without window data', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:remove'](
        { sender: { id: 1 } },
        { fileName: 'song.mp3' }
      )) as { success: boolean };
      expect(result.success).toBe(false);
    });

    it('unlinks file and returns success', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockUnlink.mockResolvedValue(undefined);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = await ipcHandle['video-editor:music:remove'](
        { sender: { id: 1 } },
        { fileName: 'song.mp3' }
      );
      expect(result).toEqual({ success: true });
    });

    it('treats ENOENT as success', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      const err = new Error('not found');
      (err as NodeJS.ErrnoException).code = 'ENOENT';
      mockUnlink.mockRejectedValue(err);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = await ipcHandle['video-editor:music:remove'](
        { sender: { id: 1 } },
        { fileName: 'song.mp3' }
      );
      expect(result).toEqual({ success: true });
    });

    it('returns error on other unlink failures', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockUnlink.mockRejectedValue(new Error('locked'));
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:remove'](
        { sender: { id: 1 } },
        { fileName: 'song.mp3' }
      )) as { success: boolean; error?: string };
      expect(result.success).toBe(false);
      expect(result.error).toBe('locked');
    });
  });

  describe('music:get-path', () => {
    it('returns null without window data', async () => {
      mockGetWindowData.mockReturnValue(undefined);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = await ipcHandle['video-editor:music:get-path'](
        { sender: { id: 1 } },
        { fileName: 'song.mp3' }
      );
      expect(result).toBeNull();
    });

    it('returns path when file exists', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockExistsSync.mockReturnValue(true);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = await ipcHandle['video-editor:music:get-path'](
        { sender: { id: 1 } },
        { fileName: 'song.mp3' }
      );
      expect(result).toBe(path.join('/p/Rec.poratake/music', 'song.mp3'));
    });

    it('returns null when file missing', async () => {
      mockGetWindowData.mockReturnValue({
        filePath: '/p/Rec.poratake/recording.mov',
      });
      mockExistsSync.mockReturnValue(false);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = await ipcHandle['video-editor:music:get-path'](
        { sender: { id: 1 } },
        { fileName: 'song.mp3' }
      );
      expect(result).toBeNull();
    });
  });

  describe('music:prepare-for-export', () => {
    it('rejects invalid speed before probing audio', async () => {
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();

      const result = (await ipcHandle['video-editor:music:prepare-for-export'](
        exportEvent,
        {
          musicFilePath: '/p/song.mp3',
          trimStart: 0,
          trimEnd: 0,
          speed: 0,
          startTime: 0,
          totalDuration: 10,
          outputPath: '/p/out.aac',
        }
      )) as { success: boolean; error?: string };

      expect(result).toEqual({
        success: false,
        error: 'Invalid music track timing',
      });
      expect(mockExecFileAsync).not.toHaveBeenCalled();
    });

    it('returns error when clipped duration is non-positive', async () => {
      mockExecFileAsync.mockImplementation(() => {
        const e = new Error('expected');
        (e as { stderr?: string }).stderr = 'Duration: 00:00:10.00, start: 0';
        throw e;
      });
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = (await ipcHandle['video-editor:music:prepare-for-export'](
        exportEvent,
        {
          musicFilePath: '/p/song.mp3',
          trimStart: 0,
          trimEnd: 0,
          speed: 1,
          startTime: 100,
          totalDuration: 5,
          outputPath: '/p/out.aac',
        }
      )) as { success: boolean; error?: string };
      expect(result.success).toBe(false);
      expect(result.error).toBe('Music track has no audible range');
    });

    it('exports music with simple trim', async () => {
      const controller = new AbortController();
      mockGetExportAbortSignal.mockReturnValue(controller.signal);
      mockExecFileAsync.mockImplementationOnce(() => {
        const e = new Error('expected');
        (e as { stderr?: string }).stderr = 'Duration: 00:00:10.00, start: 0';
        throw e;
      });
      mockExecFileAsync.mockResolvedValueOnce({ stdout: '', stderr: '' });
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      const result = await ipcHandle['video-editor:music:prepare-for-export'](
        exportEvent,
        {
          musicFilePath: '/p/song.mp3',
          trimStart: 1,
          trimEnd: 1,
          speed: 1,
          startTime: 0,
          totalDuration: 30,
          outputPath: '/p/out.aac',
        }
      );
      expect(result).toEqual({ success: true });
      expect(mockExecFileAsync.mock.calls[0][2]).toEqual({
        timeout: 10000,
        signal: controller.signal,
      });
      expect(mockExecFileAsync.mock.calls[1][2]).toEqual({
        timeout: 120000,
        signal: controller.signal,
      });
    });

    it('exports music with startTime > 0 prepends silence', async () => {
      mockExecFileAsync.mockImplementationOnce(() => {
        const e = new Error('expected');
        (e as { stderr?: string }).stderr = 'Duration: 00:00:10.00, start: 0';
        throw e;
      });
      mockExecFileAsync.mockResolvedValueOnce({ stdout: '', stderr: '' });
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();
      await ipcHandle['video-editor:music:prepare-for-export'](exportEvent, {
        musicFilePath: '/p/song.mp3',
        trimStart: 0,
        trimEnd: 0,
        speed: 1,
        startTime: 2,
        totalDuration: 30,
        outputPath: '/p/out.aac',
      });
      const args = mockExecFileAsync.mock.calls[1][1];
      expect(args.join(' ')).toContain('anullsrc');
    });

    it('removes a partial music output when export is aborted', async () => {
      mockExecFileAsync.mockImplementationOnce(() => {
        const error = new Error('expected');
        (error as { stderr?: string }).stderr =
          'Duration: 00:00:10.00, start: 0';
        throw error;
      });
      mockExecFileAsync.mockRejectedValueOnce(
        new Error('The operation was aborted')
      );
      mockUnlink.mockResolvedValue(undefined);
      const { registerMusicHandlers } =
        await import('@/main/capture/video/ipc/music-handlers');
      registerMusicHandlers();

      const result = (await ipcHandle['video-editor:music:prepare-for-export'](
        exportEvent,
        {
          musicFilePath: '/p/song.mp3',
          trimStart: 0,
          trimEnd: 0,
          speed: 1,
          startTime: 0,
          totalDuration: 30,
          outputPath: '/p/out.aac',
        }
      )) as { success: boolean };

      expect(result.success).toBe(false);
      expect(mockUnlink).toHaveBeenCalledWith('/p/out.aac');
    });
  });
});
