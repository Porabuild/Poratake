import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import path from 'path';

const mockDaemonCall = vi.fn();
const mockShowOverlay = vi.fn();
const mockHideOverlay = vi.fn();
const mockShowTray = vi.fn();
const mockHideTray = vi.fn();
const mockEnsureDirectoryExists = vi.fn();
const mockIsValidDirectory = vi.fn();
const mockGetConfig = vi.fn();
const mockGenerateFilename = vi.fn();
const mockCreateProjectFolder = vi.fn();
const mockDipToScreenRect = vi.fn();
const daemonEventHandlers = new Set<(event: string, data: unknown) => void>();

vi.mock('electron', () => ({
  app: { getPath: () => '/Users/me/Movies' },
  screen: {
    dipToScreenRect: (...a: unknown[]) => mockDipToScreenRect(...a),
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...a: unknown[]) => mockDaemonCall(...a),
    onEvent: (handler: (event: string, data: unknown) => void) =>
      daemonEventHandlers.add(handler),
    offEvent: (handler: (event: string, data: unknown) => void) =>
      daemonEventHandlers.delete(handler),
  },
}));

vi.mock('@/main/capture/video/overlay.ts', () => ({
  showRecordingOverlay: (...a: unknown[]) => mockShowOverlay(...a),
  hideRecordingOverlay: (...a: unknown[]) => mockHideOverlay(...a),
}));

vi.mock('@/main/menu/recording-tray.ts', () => ({
  showRecordingTray: () => mockShowTray(),
  hideRecordingTray: () => mockHideTray(),
}));

vi.mock('@/main/utils/paths.ts', () => ({
  ensureDirectoryExists: (...a: unknown[]) => mockEnsureDirectoryExists(...a),
  isValidDirectory: (...a: unknown[]) => mockIsValidDirectory(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
}));

vi.mock('@/main/utils/filename-generator', () => ({
  generateFilename: (...a: unknown[]) => mockGenerateFilename(...a),
}));

vi.mock('@/main/capture/video/recording-project', () => ({
  createProjectFolder: (...a: unknown[]) => mockCreateProjectFolder(...a),
}));

describe('recorder', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    daemonEventHandlers.clear();
    mockEnsureDirectoryExists.mockImplementation((p: string) => p);
    mockIsValidDirectory.mockReturnValue(false);
    mockGetConfig.mockReturnValue({
      storage: { namingPattern: 'Recording {date}' },
    });
    mockGenerateFilename.mockReturnValue('Recording 2025-01-01');
    mockDipToScreenRect.mockImplementation((_window, bounds) => bounds);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
  });

  describe('paths/naming', () => {
    it('getRecordingsDir uses Videos/Capty by default', async () => {
      const { getRecordingsDir } =
        await import('@/main/capture/video/recorder');
      expect(getRecordingsDir()).toBe(path.join('/Users/me/Movies', 'Capty'));
    });

    it('getRecordingsDir respects custom path when valid', async () => {
      mockIsValidDirectory.mockReturnValue(true);
      mockGetConfig.mockReturnValue({
        storage: { recordingsPath: '/custom/path' },
      });
      const { getRecordingsDir } =
        await import('@/main/capture/video/recorder');
      expect(getRecordingsDir()).toBe('/custom/path');
    });

    it('generateRecordingProjectName appends .capty extension', async () => {
      const { generateRecordingProjectName } =
        await import('@/main/capture/video/recorder');
      expect(generateRecordingProjectName()).toBe('Recording 2025-01-01.capty');
    });

    it('createRecordingProject delegates to createProjectFolder', async () => {
      mockCreateProjectFolder.mockReturnValue('/path/recording.mov');
      const { createRecordingProject } =
        await import('@/main/capture/video/recorder');
      const result = createRecordingProject();
      expect(result).toBe('/path/recording.mov');
      expect(mockCreateProjectFolder).toHaveBeenCalled();
    });

    it('generateRecordingExportName generates filename with extension', async () => {
      mockGenerateFilename.mockReturnValue('Recording 2025-01-01.mp4');
      const { generateRecordingExportName } =
        await import('@/main/capture/video/recorder');
      expect(generateRecordingExportName('mp4')).toBe(
        'Recording 2025-01-01.mp4'
      );
    });
  });

  describe('state getters', () => {
    it('starts idle', async () => {
      const m = await import('@/main/capture/video/recorder');
      expect(m.isRecording()).toBe(false);
      expect(m.isPaused()).toBe(false);
      expect(m.getRecordingState()).toBe('idle');
      expect(m.getRecordingDuration()).toBe(0);
      expect(m.getCurrentRecordingPath()).toBeNull();
    });
  });

  describe('startRecordingWithConfig', () => {
    it('starts recording and shows overlay for area recordings', async () => {
      mockDaemonCall.mockResolvedValue({ success: true, state: 'recording' });
      mockShowOverlay.mockResolvedValue(undefined);
      const m = await import('@/main/capture/video/recorder');
      const showControl = vi.fn();
      await m.startRecordingWithConfig(
        {
          x: 10,
          y: 20,
          width: 800,
          height: 600,
          outputPath: '/path/out.mov',
        },
        showControl
      );
      expect(showControl).toHaveBeenCalled();
      expect(mockShowOverlay).toHaveBeenCalledWith(10, 20, 800, 600);
      expect(mockShowTray).toHaveBeenCalled();
      expect(m.isRecording()).toBe(true);
    });

    it('does not show overlay for iOS recordings', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig(
        {
          x: 0,
          y: 0,
          width: 100,
          height: 100,
          iosDeviceId: 'ios-1',
          outputPath: '/out.mov',
        },
        vi.fn()
      );
      expect(mockShowOverlay).not.toHaveBeenCalled();
    });

    it('uses physical capture bounds on Windows', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      mockDaemonCall.mockResolvedValue({ success: true, state: 'recording' });
      mockShowOverlay.mockResolvedValue(undefined);
      const logicalBounds = { x: 10, y: 20, width: 800, height: 600 };
      const physicalBounds = { x: 20, y: 40, width: 1600, height: 1200 };
      mockDipToScreenRect.mockReturnValue(physicalBounds);

      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig(
        { ...logicalBounds, outputPath: '/path/out.mov' },
        vi.fn()
      );

      expect(mockDipToScreenRect).toHaveBeenCalledWith(null, logicalBounds);
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'start',
        expect.objectContaining(physicalBounds),
        60000
      );
      expect(mockShowOverlay).toHaveBeenCalledWith(20, 40, 1600, 1200);
    });

    it('throws when daemon returns failure', async () => {
      mockDaemonCall.mockResolvedValue({
        success: false,
        message: 'bad config',
      });
      const m = await import('@/main/capture/video/recorder');
      await expect(
        m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn())
      ).rejects.toThrow('bad config');
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'stop',
        undefined,
        60000
      );
      expect(mockHideOverlay).toHaveBeenCalledWith(true);
      expect(m.getRecordingState()).toBe('idle');
    });

    it('rolls back native recording when the overlay fails', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });
      mockShowOverlay.mockRejectedValue(new Error('overlay failed'));
      const m = await import('@/main/capture/video/recorder');
      const hideControl = vi.fn();

      await expect(
        m.startRecordingWithConfig(
          { x: 0, y: 0, width: 100, height: 100, outputPath: '/out.mov' },
          vi.fn(),
          hideControl
        )
      ).rejects.toThrow('overlay failed');

      expect(hideControl).toHaveBeenCalled();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'stop',
        undefined,
        60000
      );
      expect(mockHideOverlay).toHaveBeenCalledWith(true);
    });

    it('rolls back native recording when the control fails', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });
      mockShowOverlay.mockResolvedValue(undefined);
      const m = await import('@/main/capture/video/recorder');
      const hideControl = vi.fn();

      await expect(
        m.startRecordingWithConfig(
          { outputPath: '/out.mov' },
          vi.fn().mockRejectedValue(new Error('control failed')),
          hideControl
        )
      ).rejects.toThrow('control failed');

      expect(hideControl).toHaveBeenCalled();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'stop',
        undefined,
        60000
      );
      expect(m.getRecordingState()).toBe('idle');
    });

    it('catches an immediate terminal event before start resolves', async () => {
      mockDaemonCall.mockImplementation((module: string, method: string) => {
        if (module === 'screen-recorder' && method === 'start') {
          for (const handler of daemonEventHandlers) {
            handler('screen-recorder:error', {
              code: 'ENCODER_ERROR',
              message: 'Encoder failed',
            });
          }
        }
        return Promise.resolve({ success: true });
      });
      const m = await import('@/main/capture/video/recorder');
      const onFailure = vi.fn();

      await expect(
        m.startRecordingWithConfig(
          { outputPath: '/out.mov' },
          vi.fn(),
          vi.fn(),
          onFailure
        )
      ).rejects.toMatchObject({
        message: 'Encoder failed',
        code: 'ENCODER_ERROR',
      });

      expect(onFailure).not.toHaveBeenCalled();
      expect(daemonEventHandlers.size).toBe(0);
    });

    it('waits for pending startup UI before rolling back a terminal error', async () => {
      let resolveOverlay = () => {};
      mockDaemonCall.mockResolvedValue({ success: true });
      mockShowOverlay.mockReturnValue(
        new Promise<void>(resolve => {
          resolveOverlay = resolve;
        })
      );
      const m = await import('@/main/capture/video/recorder');
      const start = m.startRecordingWithConfig(
        { x: 0, y: 0, width: 100, height: 100, outputPath: '/out.mov' },
        vi.fn(),
        vi.fn()
      );
      const rejected = expect(start).rejects.toThrow('Capture failed');

      await vi.waitFor(() => expect(mockShowOverlay).toHaveBeenCalled());
      for (const handler of daemonEventHandlers) {
        handler('screen-recorder:error', {
          code: 'CAPTURE_ERROR',
          message: 'Capture failed',
        });
      }
      await Promise.resolve();

      expect(mockHideOverlay).not.toHaveBeenCalled();
      resolveOverlay();
      await rejected;
      expect(mockHideOverlay).toHaveBeenCalledWith(true);
    });

    it('cleans up and reports a terminal event after start', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });
      const m = await import('@/main/capture/video/recorder');
      const onFailure = vi.fn();
      await m.startRecordingWithConfig(
        { outputPath: '/out.mov' },
        vi.fn(),
        vi.fn(),
        onFailure
      );

      for (const handler of daemonEventHandlers) {
        handler('screen-recorder:error', {
          code: 'CAPTURE_ERROR',
          message: 'Capture failed',
        });
      }

      await vi.waitFor(() => expect(onFailure).toHaveBeenCalledTimes(1));
      expect(onFailure.mock.calls[0][0]).toMatchObject({
        message: 'Capture failed',
        code: 'CAPTURE_ERROR',
      });
      expect(m.getRecordingState()).toBe('idle');
      expect(m.getCurrentRecordingPath()).toBeNull();
      expect(mockHideOverlay).toHaveBeenCalledWith(true);
      expect(mockHideTray).toHaveBeenCalled();
      expect(daemonEventHandlers.size).toBe(0);
    });

    it('ignores a stale terminal listener after a newer start', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });
      const m = await import('@/main/capture/video/recorder');
      const firstFailure = vi.fn();
      await m.startRecordingWithConfig(
        { outputPath: '/first.mov' },
        vi.fn(),
        vi.fn(),
        firstFailure
      );
      const staleHandler = [...daemonEventHandlers][0];
      await m.stopRecording(vi.fn());

      const secondFailure = vi.fn();
      await m.startRecordingWithConfig(
        { outputPath: '/second.mov' },
        vi.fn(),
        vi.fn(),
        secondFailure
      );
      staleHandler('screen-recorder:error', {
        code: 'STALE_ERROR',
        message: 'Stale failure',
      });
      await Promise.resolve();

      expect(m.getRecordingState()).toBe('recording');
      expect(m.getCurrentRecordingPath()).toBe('/second.mov');
      expect(firstFailure).not.toHaveBeenCalled();
      expect(secondFailure).not.toHaveBeenCalled();
    });
  });

  describe('pauseRecording', () => {
    it('returns early when not recording', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.pauseRecording();
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });

    it('pauses when recording', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());
      mockDaemonCall.mockResolvedValueOnce({ success: true, duration: 12 });
      await m.pauseRecording();
      expect(m.getRecordingState()).toBe('paused');
      expect(m.getRecordingDuration()).toBe(12);
    });

    it('throws when daemon pauses fails', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());
      mockDaemonCall.mockResolvedValueOnce({
        success: false,
        message: 'fail',
      });
      await expect(m.pauseRecording()).rejects.toThrow('fail');
    });
  });

  describe('resumeRecording', () => {
    it('returns early when not paused', async () => {
      const m = await import('@/main/capture/video/recorder');
      await m.resumeRecording();
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });

    it('resumes from paused state', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      await m.pauseRecording();
      mockDaemonCall.mockResolvedValueOnce({ success: true, duration: 30 });
      await m.resumeRecording();
      expect(m.getRecordingState()).toBe('recording');
      expect(m.getRecordingDuration()).toBe(30);
    });
  });

  describe('stopRecording', () => {
    it('returns null when not recording', async () => {
      const m = await import('@/main/capture/video/recorder');
      const result = await m.stopRecording(vi.fn());
      expect(result).toBeNull();
    });

    it('stops recording and returns final path', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());
      mockDaemonCall.mockResolvedValueOnce({
        success: true,
        outputPath: '/final/out.mov',
        duration: 18,
      });
      const hideControl = vi.fn();
      const result = await m.stopRecording(hideControl);
      expect(result).toEqual({
        outputPath: '/final/out.mov',
        cursorPath: undefined,
        cameraPath: undefined,
        keysPath: undefined,
        systemAudioPath: undefined,
        micAudioPath: undefined,
        duration: 18,
      });
      expect(hideControl).toHaveBeenCalled();
      expect(mockHideOverlay).toHaveBeenCalled();
      expect(mockHideTray).toHaveBeenCalled();
      expect(m.getRecordingState()).toBe('idle');
    });

    it('hides the control before recording finalization completes', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());

      let resolveStop:
        | ((response: { success: boolean; outputPath: string }) => void)
        | undefined;
      mockDaemonCall.mockImplementationOnce(
        () =>
          new Promise(resolve => {
            resolveStop = resolve;
          })
      );

      const hideControl = vi.fn();
      const stopPromise = m.stopRecording(hideControl);

      expect(hideControl).toHaveBeenCalledTimes(1);
      await Promise.resolve();
      expect(resolveStop).toBeDefined();

      resolveStop?.({ success: true, outputPath: '/final/out.mov' });
      await expect(stopPromise).resolves.toEqual(
        expect.objectContaining({ outputPath: '/final/out.mov' })
      );
    });

    it('throws on stop failure but still resets state', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());
      mockDaemonCall.mockResolvedValueOnce({
        success: false,
        message: 'stop fail',
      });
      await expect(m.stopRecording(vi.fn())).rejects.toThrow('stop fail');
      expect(m.getRecordingState()).toBe('idle');
    });
  });

  describe('quitRecorder', () => {
    it('hides overlay and tray and idles state', async () => {
      const m = await import('@/main/capture/video/recorder');
      await m.quitRecorder();
      expect(mockHideOverlay).toHaveBeenCalled();
      expect(mockHideTray).toHaveBeenCalled();
      expect(m.getRecordingState()).toBe('idle');
    });

    it('attempts to stop daemon when recording', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      await m.quitRecorder();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'stop',
        undefined,
        5000
      );
    });

    it('swallows daemon errors during quit', async () => {
      mockDaemonCall.mockResolvedValueOnce({ success: true });
      const m = await import('@/main/capture/video/recorder');
      await m.startRecordingWithConfig({ outputPath: '/out.mov' }, vi.fn());
      mockDaemonCall.mockRejectedValueOnce(new Error('boom'));
      await expect(m.quitRecorder()).resolves.toBeUndefined();
    });
  });

  describe('prewarmRecorder', () => {
    it('calls daemon status', async () => {
      mockDaemonCall.mockResolvedValue({});
      const { prewarmRecorder } = await import('@/main/capture/video/recorder');
      await prewarmRecorder();
      expect(mockDaemonCall).toHaveBeenCalledWith(
        'screen-recorder',
        'status',
        undefined,
        5000
      );
    });

    it('swallows daemon errors', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const { prewarmRecorder } = await import('@/main/capture/video/recorder');
      await expect(prewarmRecorder()).resolves.toBeUndefined();
    });
  });
});
