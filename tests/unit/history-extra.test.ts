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
      init();
    }

    it('history:confirmClear returns true on confirm', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 0 });
      await loadInit();
      const result = await ipcHandle['history:confirmClear']({ sender: {} });
      expect(result).toBe(true);
    });

    it('history:confirmClear returns false on cancel', async () => {
      mockShowMessageBox.mockResolvedValue({ response: 1 });
      await loadInit();
      const result = await ipcHandle['history:confirmClear']({ sender: {} });
      expect(result).toBe(false);
    });

    it('history:clear clears history', async () => {
      mockWriteFile.mockResolvedValue(undefined);
      await loadInit();
      const result = await ipcHandle['history:clear']();
      expect(result).toBe(true);
    });

    it('history:getThumbnail returns base64', async () => {
      mockGetThumbnail.mockResolvedValue({ base64: 'abc', cached: false });
      await loadInit();
      const result = await ipcHandle['history:getThumbnail'](
        {},
        '/p/x.png',
        'screenshot'
      );
      expect(result).toBe('abc');
    });

    it('history:getVideoFeatures returns features', async () => {
      mockGetProjectFolder.mockReturnValue(null);
      await loadInit();
      const result = await ipcHandle['history:getVideoFeatures'](
        {},
        '/p/x.mov'
      );
      expect(result).toEqual({
        hasMic: false,
        hasSystemAudio: false,
        hasCamera: false,
        hasCursor: false,
      });
    });
  });
});
