import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockGetWindowData = vi.fn();
const mockShowOpenDialog = vi.fn();
const mockReadFile = vi.fn();
const mockValidateCursorData = vi.fn();
const mockSaveCursorData = vi.fn();
const mockBrowserWindowFromWebContents = vi.fn();
const mockExecFileAsync = vi.fn();

vi.mock('electron', () => ({
  ipcMain: {
    handle: (e: string, h: Handler) => {
      ipcHandle[e] = h;
    },
  },
  BrowserWindow: {
    fromWebContents: (...a: unknown[]) =>
      mockBrowserWindowFromWebContents(...a),
  },
  dialog: { showOpenDialog: (...a: unknown[]) => mockShowOpenDialog(...a) },
}));

vi.mock('fs', () => ({
  default: { existsSync: vi.fn(() => false) },
  existsSync: vi.fn(() => false),
}));

vi.mock('fs/promises', () => ({
  default: { readFile: (...a: unknown[]) => mockReadFile(...a) },
  readFile: (...a: unknown[]) => mockReadFile(...a),
}));

vi.mock('child_process', () => ({ execFile: vi.fn() }));

vi.mock('util', () => ({
  promisify:
    () =>
    (...a: unknown[]) =>
      mockExecFileAsync(...a),
}));

vi.mock('@/main/capture/video/window-manager', () => ({
  getWindowData: (...a: unknown[]) => mockGetWindowData(...a),
}));

vi.mock('@/main/capture/video/cursor-data', () => ({
  loadCursorData: vi.fn(),
  saveCursorData: (...a: unknown[]) => mockSaveCursorData(...a),
}));

vi.mock('@/main/capture/video/camera-data', () => ({
  loadCameraData: vi.fn(),
  getAbsoluteCameraVideoPath: vi.fn(),
}));

vi.mock('@/main/capture/video/keyboard-data', () => ({
  loadKeyboardData: vi.fn(),
}));

vi.mock('@/main/capture/video/recording-project', () => ({
  getSystemAudioPath: (p: string) => `${p}.system`,
  getMicAudioPath: (p: string) => `${p}.mic`,
}));

vi.mock('@/main/utils/ffmpeg', () => ({
  getFFmpegPath: () => '/bin/ffmpeg',
}));

vi.mock('@/types/cursor', () => ({
  validateCursorData: (...a: unknown[]) => mockValidateCursorData(...a),
}));

describe('data-handlers extra', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
  });

  it('saveCursorData returns error on save throw', async () => {
    mockGetWindowData.mockReturnValue({ filePath: '/p/x.mov' });
    mockValidateCursorData.mockReturnValue({ valid: true });
    mockSaveCursorData.mockRejectedValue(new Error('disk full'));
    const { registerDataHandlers } =
      await import('@/main/capture/video/ipc/data-handlers');
    registerDataHandlers();
    const result = (await ipcHandle['video-editor:saveCursorData'](
      { sender: { id: 1 } },
      {}
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('disk full');
  });

  it('importCursorData returns error when no window data', async () => {
    mockGetWindowData.mockReturnValue(undefined);
    const { registerDataHandlers } =
      await import('@/main/capture/video/ipc/data-handlers');
    registerDataHandlers();
    const result = (await ipcHandle['video-editor:importCursorData']({
      sender: { id: 1 },
    })) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('No video data found');
  });

  it('importCursorData returns error when no window from webContents', async () => {
    mockGetWindowData.mockReturnValue({ filePath: '/p/x.mov' });
    mockBrowserWindowFromWebContents.mockReturnValue(null);
    const { registerDataHandlers } =
      await import('@/main/capture/video/ipc/data-handlers');
    registerDataHandlers();
    const result = (await ipcHandle['video-editor:importCursorData']({
      sender: { id: 1 },
    })) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('Window not found');
  });

  it('importCursorData imports valid data', async () => {
    mockGetWindowData.mockReturnValue({ filePath: '/p/x.mov' });
    mockBrowserWindowFromWebContents.mockReturnValue({ id: 1 });
    mockShowOpenDialog.mockResolvedValue({
      canceled: false,
      filePaths: ['/p/cursor.json'],
    });
    mockReadFile.mockResolvedValue('{"events":[]}');
    mockValidateCursorData.mockReturnValue({
      valid: true,
      data: { events: [] },
    });
    mockSaveCursorData.mockResolvedValue(undefined);
    const { registerDataHandlers } =
      await import('@/main/capture/video/ipc/data-handlers');
    registerDataHandlers();
    const result = (await ipcHandle['video-editor:importCursorData']({
      sender: { id: 1 },
    })) as { success: boolean; data?: unknown };
    expect(result.success).toBe(true);
  });

  it('importCursorData rejects invalid JSON', async () => {
    mockGetWindowData.mockReturnValue({ filePath: '/p/x.mov' });
    mockBrowserWindowFromWebContents.mockReturnValue({ id: 1 });
    mockShowOpenDialog.mockResolvedValue({
      canceled: false,
      filePaths: ['/p/cursor.json'],
    });
    mockReadFile.mockResolvedValue('not json');
    const { registerDataHandlers } =
      await import('@/main/capture/video/ipc/data-handlers');
    registerDataHandlers();
    const result = (await ipcHandle['video-editor:importCursorData']({
      sender: { id: 1 },
    })) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
  });

  it('importCursorData rejects invalid cursor data', async () => {
    mockGetWindowData.mockReturnValue({ filePath: '/p/x.mov' });
    mockBrowserWindowFromWebContents.mockReturnValue({ id: 1 });
    mockShowOpenDialog.mockResolvedValue({
      canceled: false,
      filePaths: ['/p/cursor.json'],
    });
    mockReadFile.mockResolvedValue('{}');
    mockValidateCursorData.mockReturnValue({
      valid: false,
      error: 'invalid',
    });
    const { registerDataHandlers } =
      await import('@/main/capture/video/ipc/data-handlers');
    registerDataHandlers();
    const result = (await ipcHandle['video-editor:importCursorData']({
      sender: { id: 1 },
    })) as { success: boolean; error?: string };
    expect(result.error).toBe('invalid');
  });

  it('getAudioPaths probes for embedded audio when no separate files', async () => {
    mockGetWindowData.mockReturnValue({ filePath: '/p/x.mov' });
    mockExecFileAsync.mockRejectedValue(
      Object.assign(new Error('fail'), {
        stderr: '  Stream: Audio: aac, 44100 Hz',
      })
    );
    const { registerDataHandlers } =
      await import('@/main/capture/video/ipc/data-handlers');
    registerDataHandlers();
    const result = (await ipcHandle['video-editor:getAudioPaths']({
      sender: { id: 1 },
    })) as { hasEmbeddedAudio: boolean };
    expect(result.hasEmbeddedAudio).toBe(true);
  });
});
