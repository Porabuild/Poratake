import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockExistsSync = vi.fn();
const mockMkdirSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockWriteFileSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockReadFile = vi.fn();
const mockWriteFile = vi.fn();
const mockUnlink = vi.fn();
const mockShowMessageBox = vi.fn();
const mockGetThumbnail = vi.fn();
const mockDeleteThumbnail = vi.fn();
const mockGetConfig = vi.fn();
const mockGetProjectFolder = vi.fn();
const mockGetMicAudioPath = vi.fn();
const mockGetSystemAudioPath = vi.fn();
const mockGetCameraVideoPath = vi.fn();
const mockGetCursorPath = vi.fn();
const mockIsHistoryPopoverWebContents = vi.fn(() => true);

vi.mock('electron', () => ({
  app: {
    isPackaged: false,
    getPath: () => '/home',
    getAppPath: () => '/app',
  },
  ipcMain: {
    handle: (e: string, h: Handler) => {
      ipcHandle[e] = h;
    },
  },
  BrowserWindow: { fromWebContents: () => null },
  dialog: { showMessageBox: (...a: unknown[]) => mockShowMessageBox(...a) },
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
    readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
    writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
  writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
}));

vi.mock('fs/promises', () => ({
  default: {
    readFile: (...a: unknown[]) => mockReadFile(...a),
    writeFile: (...a: unknown[]) => mockWriteFile(...a),
    unlink: (...a: unknown[]) => mockUnlink(...a),
  },
  readFile: (...a: unknown[]) => mockReadFile(...a),
  writeFile: (...a: unknown[]) => mockWriteFile(...a),
  unlink: (...a: unknown[]) => mockUnlink(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/utils/thumbnails', () => ({
  getThumbnail: (...a: unknown[]) => mockGetThumbnail(...a),
  deleteThumbnail: (...a: unknown[]) => mockDeleteThumbnail(...a),
  clearAllThumbnails: vi.fn(),
  rekeyThumbnail: vi.fn(),
}));

vi.mock('@/main/capture/video/recording-project', () => ({
  getProjectFolder: (...a: unknown[]) => mockGetProjectFolder(...a),
  getMicAudioPath: (...a: unknown[]) => mockGetMicAudioPath(...a),
  getSystemAudioPath: (...a: unknown[]) => mockGetSystemAudioPath(...a),
  getCameraVideoPath: (...a: unknown[]) => mockGetCameraVideoPath(...a),
  getCursorPath: (...a: unknown[]) => mockGetCursorPath(...a),
}));

vi.mock('@/main/history/popover', () => ({
  preloadHistoryPopover: vi.fn(),
  showHistoryPopover: vi.fn(),
  closeHistoryPopover: vi.fn(),
  toggleHistoryPopover: vi.fn(),
  getHistoryPopover: () => null,
  isHistoryPopoverWebContents: (...a: unknown[]) =>
    mockIsHistoryPopoverWebContents(...a),
  isHistoryPopoverVisible: () => false,
}));

describe('history extra', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockGetConfig.mockReturnValue({
      history: { enabled: true, maxItems: 100 },
    });
    mockGetMicAudioPath.mockImplementation((p: string) => `${p}.mic`);
    mockGetSystemAudioPath.mockImplementation((p: string) => `${p}.sys`);
    mockGetCameraVideoPath.mockImplementation((p: string) => `${p}.cam`);
    mockGetCursorPath.mockImplementation((p: string) => `${p}.cur`);
    mockIsHistoryPopoverWebContents.mockReturnValue(true);
  });

  describe('getVideoRecordingFeatures', () => {
    it('returns all false when not in a project', async () => {
      mockGetProjectFolder.mockReturnValue(null);
      const { getVideoRecordingFeatures } = await import('@/main/history');
      const result = getVideoRecordingFeatures('/p/video.mov');
      expect(result).toEqual({
        hasMic: false,
        hasSystemAudio: false,
        hasCamera: false,
        hasCursor: false,
      });
    });

    it('returns feature flags based on file existence', async () => {
      mockGetProjectFolder.mockReturnValue('/p/Rec.capty');
      mockExistsSync.mockImplementation(
        (p: string) => String(p).endsWith('.cam') || String(p).endsWith('.cur')
      );
      const { getVideoRecordingFeatures } = await import('@/main/history');
      const result = getVideoRecordingFeatures('/p/Rec.capty/recording.mov');
      expect(result).toEqual({
        hasMic: false,
        hasSystemAudio: false,
        hasCamera: true,
        hasCursor: true,
      });
    });
  });

  describe('init IPC handlers', () => {
    async function loadInit(): Promise<void> {
      mockExistsSync.mockReturnValue(false);
      const { init } = await import('@/main/history');
      await init();
    }

    it('history:clear clears history on confirm', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      mockWriteFile.mockResolvedValue(undefined);
      await loadInit();
      const result = await ipcHandle['history:clear']({ sender: {} });
      expect(result).toBe(true);
    });

    it('history:clear preserves history on cancel', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      await loadInit();
      const result = await ipcHandle['history:clear']({ sender: {} });
      expect(result).toBe(false);
      expect(mockWriteFile).not.toHaveBeenCalled();
    });

    it('history:getThumbnail returns base64', async () => {
      mockGetThumbnail.mockResolvedValue({ base64: 'abc', cached: false });
      mockExistsSync.mockReturnValue(true);
      mockReadFile.mockResolvedValue(
        JSON.stringify([
          {
            id: 'h1',
            timestamp: 1,
            originalPath: '/p/x.png',
            type: 'screenshot',
            editorState: null,
          },
        ])
      );
      const { init } = await import('@/main/history');
      await init();
      const result = await ipcHandle['history:getThumbnail'](
        { sender: {} },
        'h1'
      );
      expect(result).toBe('abc');
      expect(mockGetThumbnail).toHaveBeenCalledWith('/p/x.png', 'screenshot');
    });

    it('history:getVideoFeatures rejects an unknown history id', async () => {
      mockGetProjectFolder.mockReturnValue(null);
      await loadInit();
      const result = await ipcHandle['history:getVideoFeatures'](
        { sender: {} },
        'missing'
      );
      expect(result).toEqual({
        hasMic: false,
        hasSystemAudio: false,
        hasCamera: false,
        hasCursor: false,
      });
    });

    it('history:getVideoFeatures resolves the stored video path by id', async () => {
      mockReadFile.mockResolvedValue(
        JSON.stringify([
          {
            id: 'v1',
            timestamp: 1,
            originalPath: '/p/video.mov',
            type: 'video',
            editorState: null,
          },
        ])
      );
      mockGetProjectFolder.mockReturnValue('/p/video.capty');
      mockExistsSync.mockImplementation((path: string) => {
        const value = String(path);
        return (
          value.endsWith('history.json') ||
          value === '/p/video.mov' ||
          value.endsWith('.cam')
        );
      });
      const { init } = await import('@/main/history');
      await init();

      const result = await ipcHandle['history:getVideoFeatures'](
        { sender: {} },
        'v1'
      );

      expect(result).toEqual({
        hasMic: false,
        hasSystemAudio: false,
        hasCamera: true,
        hasCursor: false,
      });
    });

    it('rejects history access from another renderer', async () => {
      mockIsHistoryPopoverWebContents.mockReturnValue(false);
      await loadInit();

      expect(ipcHandle['history:get']({ sender: {} })).toEqual([]);
      await expect(
        ipcHandle['history:delete']({ sender: {} }, 'h1')
      ).resolves.toBe(false);
      await expect(ipcHandle['history:clear']({ sender: {} })).resolves.toBe(
        false
      );
      await expect(
        ipcHandle['history:getThumbnail']({ sender: {} }, 'h1')
      ).resolves.toBeNull();
      expect(mockShowMessageBox).not.toHaveBeenCalled();
      expect(mockWriteFile).not.toHaveBeenCalled();
    });
  });
});
