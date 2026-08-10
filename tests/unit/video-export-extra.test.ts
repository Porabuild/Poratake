import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import path from 'path';

type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockExistsSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockReadFileSync = vi.fn();
const mockProbeVideo = vi.fn();
const mockClipboardWriteBuffer = vi.fn();
const mockLoadCursorData = vi.fn();
const mockLoadCameraData = vi.fn();
const mockGetAbsoluteCameraVideoPath = vi.fn();
const mockAuthorizeExportOutputPaths = vi.fn();
const mockIsExportOutputPathAllowed = vi.fn(() => true);
const mockExecFile = vi.fn((...args: unknown[]) => {
  const callback = args[3] as (error: Error | null) => void;
  callback(null);
});
const previewEvent = { sender: { id: 1 } };
const originalPlatform = process.platform;

vi.mock('electron', () => ({
  ipcMain: {
    handle: (e: string, h: Handler) => {
      ipcHandle[e] = h;
    },
  },
  clipboard: {
    writeBuffer: (...a: unknown[]) => mockClipboardWriteBuffer(...a),
  },
  app: { getPath: () => '/tmp' },
}));

vi.mock('fs', () => ({
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
}));

vi.mock('child_process', () => ({
  execFile: (...args: unknown[]) => mockExecFile(...args),
}));

vi.mock('@/main/utils/ffmpeg', () => ({
  probeVideo: (...a: unknown[]) => mockProbeVideo(...a),
}));

vi.mock('@/main/capture/video/recording-project', () => ({
  isRecordingProject: (p: string) => p.includes('.capty'),
  getRecordingVideoPath: (p: string) =>
    p.includes('.capty') ? `${p}/recording.mov` : p,
  getSystemAudioPath: (p: string) => `${p}.system.m4a`,
  getMicAudioPath: (p: string) => `${p}.mic.m4a`,
  getEditorStatePath: (p: string) =>
    p.includes('.capty') ? `${p}/state.json` : null,
}));

vi.mock('@/main/capture/video/cursor-data', () => ({
  loadCursorData: (...a: unknown[]) => mockLoadCursorData(...a),
}));

vi.mock('@/main/capture/video/camera-data', () => ({
  loadCameraData: (...a: unknown[]) => mockLoadCameraData(...a),
  getAbsoluteCameraVideoPath: (...a: unknown[]) =>
    mockGetAbsoluteCameraVideoPath(...a),
}));

vi.mock('@/main/capture/video/ipc/export-session', () => ({
  authorizeExportOutputPaths: (...a: unknown[]) =>
    mockAuthorizeExportOutputPaths(...a),
  isExportOutputPathAllowed: (...a: unknown[]) =>
    mockIsExportOutputPathAllowed(...a),
}));

describe('capture-preview video-export', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    vi.useFakeTimers();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockIsExportOutputPathAllowed.mockReturnValue(true);
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  describe('load-export-data', () => {
    it('returns null when probe fails', async () => {
      mockProbeVideo.mockResolvedValue(null);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');
      const result = await ipcHandle['capture-preview:load-export-data'](
        previewEvent,
        '/p/unrelated.mov'
      );
      expect(result).toBeNull();
      expect(mockProbeVideo).toHaveBeenCalledWith('/p/video.mov');
    });

    it('returns payload with paths when files exist', async () => {
      mockProbeVideo.mockResolvedValue({
        metadata: { width: 1920, height: 1080, duration: 10 },
        hasAudio: false,
      });
      mockLoadCursorData.mockResolvedValue({ events: [] });
      mockLoadCameraData.mockResolvedValue({ videoFile: 'camera.mov' });
      mockGetAbsoluteCameraVideoPath.mockReturnValue('/p/camera.mov');
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(
        JSON.stringify({
          segments: [{ id: 's1' }],
          zoomSegments: [],
          zoomSettings: { enabled: true },
        })
      );
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/Rec.capty');
      const result = (await ipcHandle['capture-preview:load-export-data'](
        previewEvent
      )) as Record<string, unknown>;
      expect(result.videoPath).toBe('/p/Rec.capty/recording.mov');
      expect(result.cameraVideoPath).toBe('/p/camera.mov');
    });

    it('hasEmbeddedAudio when no separate audio files', async () => {
      mockProbeVideo.mockResolvedValue({
        metadata: {},
        hasAudio: true,
      });
      mockLoadCursorData.mockResolvedValue(null);
      mockLoadCameraData.mockResolvedValue(null);
      mockExistsSync.mockReturnValue(false);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');
      const result = (await ipcHandle['capture-preview:load-export-data'](
        previewEvent
      )) as Record<string, unknown>;
      expect(result.hasEmbeddedAudio).toBe(true);
    });

    it('treats invalid editor state JSON as null', async () => {
      mockProbeVideo.mockResolvedValue({
        metadata: {},
        hasAudio: false,
      });
      mockLoadCursorData.mockResolvedValue(null);
      mockLoadCameraData.mockResolvedValue(null);
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue('not-json');
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/Rec.capty');
      const result = (await ipcHandle['capture-preview:load-export-data'](
        previewEvent
      )) as Record<string, unknown>;
      expect(result.segments).toBeNull();
    });
  });

  describe('get-export-output-path', () => {
    it('returns and authorizes a unique temp path', async () => {
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');
      const result =
        ipcHandle['capture-preview:get-export-output-path'](previewEvent);
      const prefix = path
        .join('/tmp', 'capty-clipboard-')
        .replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      expect(result).toMatch(new RegExp(`^${prefix}.*\\.mp4$`));
      expect(mockAuthorizeExportOutputPaths).toHaveBeenCalledWith(
        previewEvent.sender,
        [result]
      );
    });
  });

  describe('copy-video-to-clipboard', () => {
    it('rejects paths outside the active export', async () => {
      mockIsExportOutputPathAllowed.mockReturnValue(false);
      mockExistsSync.mockReturnValue(true);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');

      const result = await ipcHandle['capture-preview:copy-video-to-clipboard'](
        previewEvent,
        '/p/unrelated.mp4'
      );

      expect(result).toBe(false);
      expect(mockClipboardWriteBuffer).not.toHaveBeenCalled();
    });

    it('returns false when output missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');
      const result = await ipcHandle['capture-preview:copy-video-to-clipboard'](
        previewEvent,
        '/p/out.mp4'
      );
      expect(result).toBe(false);
    });

    it('writes file URL buffer to clipboard', async () => {
      mockExistsSync.mockReturnValue(true);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');
      const result = await ipcHandle['capture-preview:copy-video-to-clipboard'](
        previewEvent,
        '/p/out.mp4'
      );
      expect(result).toBe(true);
      expect(mockClipboardWriteBuffer).toHaveBeenCalledWith(
        'public.file-url',
        expect.any(Buffer)
      );
    });

    it('encodes Windows paths as valid file URLs', async () => {
      mockExistsSync.mockReturnValue(true);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => 'C:\\recording.mov');

      await ipcHandle['capture-preview:copy-video-to-clipboard'](
        previewEvent,
        'C:\\Users\\Test User\\clip #1.mp4'
      );

      expect(mockClipboardWriteBuffer).toHaveBeenCalledWith(
        'public.file-url',
        Buffer.from('file:///C:/Users/Test%20User/clip%20%231.mp4')
      );
    });

    it('uses the Windows file-drop clipboard path', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      mockExistsSync.mockReturnValue(true);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => 'C:\\recording.mov');

      const result = await ipcHandle['capture-preview:copy-video-to-clipboard'](
        previewEvent,
        'C:\\Users\\Test User\\clip #1.mp4'
      );

      expect(result).toBe(true);
      expect(mockClipboardWriteBuffer).not.toHaveBeenCalled();
      expect(mockExecFile).toHaveBeenCalledWith(
        'powershell.exe',
        [
          '-NoProfile',
          '-NonInteractive',
          '-Command',
          'Set-Clipboard -LiteralPath $env:PORATAKE_CLIPBOARD_FILE',
        ],
        expect.objectContaining({
          env: expect.objectContaining({
            PORATAKE_CLIPBOARD_FILE: 'C:\\Users\\Test User\\clip #1.mp4',
          }),
          windowsHide: true,
        }),
        expect.any(Function)
      );
    });

    it('schedules cleanup that unlinks file', async () => {
      mockExistsSync.mockReturnValue(true);
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');
      await ipcHandle['capture-preview:copy-video-to-clipboard'](
        previewEvent,
        '/p/out.mp4'
      );
      vi.advanceTimersByTime(5 * 60 * 1000);
      expect(mockUnlinkSync).toHaveBeenCalledWith('/p/out.mp4');
    });

    it('returns false when clipboard.writeBuffer throws', async () => {
      mockExistsSync.mockReturnValue(true);
      mockClipboardWriteBuffer.mockImplementation(() => {
        throw new Error('clipboard busy');
      });
      const { registerPreviewExportIpc } =
        await import('@/main/capture/capture-preview/video-export');
      registerPreviewExportIpc(() => '/p/video.mov');
      const result = await ipcHandle['capture-preview:copy-video-to-clipboard'](
        previewEvent,
        '/p/out.mp4'
      );
      expect(result).toBe(false);
    });
  });
});
