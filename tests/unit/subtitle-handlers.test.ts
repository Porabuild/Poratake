import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockGetWindowData = vi.fn();
const mockExistsSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockWriteFileSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockShowOpenDialog = vi.fn();
const mockIsWhisperBinaryAvailable = vi.fn();
const mockIsWhisperModelAvailable = vi.fn();
const mockGetAvailableModels = vi.fn();
const mockEnsureWhisperReady = vi.fn();
const mockTranscribeAudio = vi.fn();
const mockValidateSubtitleData = vi.fn();

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
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
    writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
  writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
}));

vi.mock('@/main/capture/video/window-manager', () => ({
  getWindowData: (...a: unknown[]) => mockGetWindowData(...a),
}));

vi.mock('@/main/capture/video/recording-project', () => ({
  getMicAudioPath: (p: string) => `${p}.mic.m4a`,
  getSubtitlePath: (p: string) =>
    p.includes('.poratake') ? `${p}/subtitle.json` : null,
}));

vi.mock('@/main/utils/whisper', () => ({
  isWhisperBinaryAvailable: () => mockIsWhisperBinaryAvailable(),
  isWhisperModelAvailable: (...a: unknown[]) =>
    mockIsWhisperModelAvailable(...a),
  ensureWhisperReady: (...a: unknown[]) => mockEnsureWhisperReady(...a),
  getAvailableModels: () => mockGetAvailableModels(),
}));

vi.mock('@/main/transcription/whisper-transcribe', () => ({
  transcribeAudio: (...a: unknown[]) => mockTranscribeAudio(...a),
}));

vi.mock('@/types/subtitle', () => ({
  validateSubtitleData: (...a: unknown[]) => mockValidateSubtitleData(...a),
}));

describe('subtitle handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
  });

  it('getWhisperStatus returns binary + models', async () => {
    mockIsWhisperBinaryAvailable.mockReturnValue(true);
    mockGetAvailableModels.mockReturnValue(['base', 'small']);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:getWhisperStatus']();
    expect(result).toEqual({
      binaryAvailable: true,
      availableModels: ['base', 'small'],
    });
  });

  it('isWhisperModelAvailable forwards to whisper utility', async () => {
    mockIsWhisperModelAvailable.mockReturnValue(true);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:isWhisperModelAvailable'](
      {},
      'base'
    );
    expect(result).toBe(true);
  });

  it('downloadWhisper returns success', async () => {
    mockEnsureWhisperReady.mockResolvedValue(undefined);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:downloadWhisper'](
      { sender: { send: vi.fn() } },
      'base'
    );
    expect(result).toEqual({ success: true });
  });

  it('downloadWhisper returns error on failure', async () => {
    mockEnsureWhisperReady.mockRejectedValue(new Error('network'));
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = (await ipcHandle['video-editor:downloadWhisper'](
      { sender: { send: vi.fn() } },
      'base'
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
    expect(result.error).toBe('network');
  });

  it('generateSubtitles errors without window data', async () => {
    mockGetWindowData.mockReturnValue(undefined);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = (await ipcHandle['video-editor:generateSubtitles'](
      { sender: { id: 1, send: vi.fn() } },
      {}
    )) as { success: boolean; error?: string };
    expect(result.error).toBe('No video loaded');
  });

  it('generateSubtitles errors when mic audio missing', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockExistsSync.mockReturnValue(false);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = (await ipcHandle['video-editor:generateSubtitles'](
      { sender: { id: 1, send: vi.fn() } },
      {}
    )) as { success: boolean; error?: string };
    expect(result.error).toBe('No microphone audio found');
  });

  it('generateSubtitles writes subtitle file on success', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockExistsSync.mockReturnValue(true);
    mockTranscribeAudio.mockResolvedValue({
      success: true,
      data: { segments: [] },
    });
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:generateSubtitles'](
      { sender: { id: 1, send: vi.fn() } },
      {}
    );
    expect(mockWriteFileSync).toHaveBeenCalled();
    expect((result as { success: boolean }).success).toBe(true);
  });

  it('getSubtitleData returns null when no window data', async () => {
    mockGetWindowData.mockReturnValue(undefined);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    expect(
      await ipcHandle['video-editor:getSubtitleData']({ sender: { id: 1 } })
    ).toBeNull();
  });

  it('getSubtitleData returns parsed JSON', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockExistsSync.mockReturnValue(true);
    mockReadFileSync.mockReturnValue(
      JSON.stringify({ segments: [], meta: {} })
    );
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:getSubtitleData']({
      sender: { id: 1 },
    });
    expect(result).toEqual({ segments: [], meta: {} });
  });

  it('getSubtitleData returns null on parse error', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockExistsSync.mockReturnValue(true);
    mockReadFileSync.mockReturnValue('bad json');
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    expect(
      await ipcHandle['video-editor:getSubtitleData']({ sender: { id: 1 } })
    ).toBeNull();
  });

  it('saveSubtitleData writes JSON', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:saveSubtitleData'](
      { sender: { id: 1 } },
      { segments: [] }
    );
    expect(result).toEqual({ success: true });
    expect(mockWriteFileSync).toHaveBeenCalled();
  });

  it('saveSubtitleData errors without window data', async () => {
    mockGetWindowData.mockReturnValue(undefined);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = (await ipcHandle['video-editor:saveSubtitleData'](
      { sender: { id: 1 } },
      {}
    )) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
  });

  it('importSubtitleData errors without window data', async () => {
    mockGetWindowData.mockReturnValue(undefined);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = (await ipcHandle['video-editor:importSubtitleData']({
      sender: { id: 1 },
    })) as { success: boolean; error?: string };
    expect(result.success).toBe(false);
  });

  it('importSubtitleData returns Cancelled when dialog cancelled', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockShowOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = (await ipcHandle['video-editor:importSubtitleData']({
      sender: { id: 1 },
    })) as { success: boolean; error?: string };
    expect(result.error).toBe('Cancelled');
  });

  it('importSubtitleData parses .srt files', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockShowOpenDialog.mockResolvedValue({
      canceled: false,
      filePaths: ['/p/sub.srt'],
    });
    mockReadFileSync.mockReturnValue(
      `1\n00:00:00,000 --> 00:00:05,000\nHello world\n\n2\n00:00:05,000 --> 00:00:10,000\nGoodbye`
    );
    mockValidateSubtitleData.mockReturnValue({
      valid: true,
      data: { segments: [] },
    });
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:importSubtitleData']({
      sender: { id: 1 },
    });
    expect((result as { success: boolean }).success).toBe(true);
  });

  it('importSubtitleData parses .json files', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockShowOpenDialog.mockResolvedValue({
      canceled: false,
      filePaths: ['/p/sub.json'],
    });
    mockReadFileSync.mockReturnValue(JSON.stringify({ segments: [] }));
    mockValidateSubtitleData.mockReturnValue({
      valid: true,
      data: { segments: [] },
    });
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:importSubtitleData']({
      sender: { id: 1 },
    });
    expect((result as { success: boolean }).success).toBe(true);
  });

  it('importSubtitleData rejects invalid subtitle data', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockShowOpenDialog.mockResolvedValue({
      canceled: false,
      filePaths: ['/p/sub.json'],
    });
    mockReadFileSync.mockReturnValue('{}');
    mockValidateSubtitleData.mockReturnValue({
      valid: false,
      error: 'bad',
    });
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = (await ipcHandle['video-editor:importSubtitleData']({
      sender: { id: 1 },
    })) as { success: boolean; error?: string };
    expect(result.error).toBe('bad');
  });

  it('deleteSubtitleData returns true when no file exists', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockExistsSync.mockReturnValue(false);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:deleteSubtitleData']({
      sender: { id: 1 },
    });
    expect(result).toBe(true);
  });

  it('deleteSubtitleData unlinks existing file', async () => {
    mockGetWindowData.mockReturnValue({
      filePath: '/p/Rec.poratake/recording.mov',
    });
    mockExistsSync.mockReturnValue(true);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:deleteSubtitleData']({
      sender: { id: 1 },
    });
    expect(result).toBe(true);
    expect(mockUnlinkSync).toHaveBeenCalled();
  });

  it('deleteSubtitleData returns false when no window data', async () => {
    mockGetWindowData.mockReturnValue(undefined);
    const { registerSubtitleHandlers } =
      await import('@/main/capture/video/ipc/subtitle-handlers');
    registerSubtitleHandlers();
    const result = await ipcHandle['video-editor:deleteSubtitleData']({
      sender: { id: 1 },
    });
    expect(result).toBe(false);
  });
});
