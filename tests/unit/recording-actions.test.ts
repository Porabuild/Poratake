import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockShowRecordingControl = vi.fn();
const mockHideRecordingControl = vi.fn();
const mockHidePreRecordingControl = vi.fn();
const mockPrewarmRecordingControl = vi.fn();
const mockShowPreRecordingControl = vi.fn();
const mockUpdatePosition = vi.fn();
const mockShowCameraPreview = vi.fn();
const mockHideCameraPreview = vi.fn();
const mockEnableCameraProtection = vi.fn();
const mockDisableCameraProtection = vi.fn();
const mockStartAreaSelection = vi.fn();
const mockConfirmAreaSelection = vi.fn();
const mockCancelAreaSelection = vi.fn();
const mockKillAreaSelector = vi.fn();
const mockIsRecording = vi.fn();
const mockGetRecordingDuration = vi.fn(() => 5);
const mockGetCurrentRecordingPath = vi.fn();
const mockStartRecordingWithConfig = vi.fn();
const mockStopRecording = vi.fn();
const mockCreateRecordingProject = vi.fn();
const mockPrewarmRecorder = vi.fn();
const mockPrewarmOverlay = vi.fn();
const mockCreateVideoEditorWindow = vi.fn();
const mockShowCapturePreview = vi.fn();
const mockPrepareCapturePreview = vi.fn();
const mockPrewarmCapturePreview = vi.fn();
const mockAddToHistory = vi.fn();
const mockShowRecordingError = vi.fn();
const mockCheckMic = vi.fn();
const mockDeleteVideo = vi.fn();
const mockGetConfig = vi.fn();
const mockUpdateConfig = vi.fn();
const mockGenerateInitialEditorState = vi.fn();

vi.mock('@/main/capture/video/recording-control.ts', () => ({
  showRecordingControl: () => mockShowRecordingControl(),
  hideRecordingControl: () => mockHideRecordingControl(),
  hidePreRecordingControl: (...a: unknown[]) =>
    mockHidePreRecordingControl(...a),
  prewarmRecordingControlWindow: () => mockPrewarmRecordingControl(),
  showPreRecordingControl: (...a: unknown[]) =>
    mockShowPreRecordingControl(...a),
  updateRecordingControlPosition: (...a: unknown[]) => mockUpdatePosition(...a),
}));

vi.mock('@/main/capture/video/camera-preview.ts', () => ({
  showCameraPreview: (...a: unknown[]) => mockShowCameraPreview(...a),
  hideCameraPreview: () => mockHideCameraPreview(),
  enableCameraContentProtection: () => mockEnableCameraProtection(),
  disableCameraContentProtection: () => mockDisableCameraProtection(),
}));

vi.mock('@/main/capture/area-selector', () => ({
  startAreaSelection: (...a: unknown[]) => mockStartAreaSelection(...a),
  confirmAreaSelection: () => mockConfirmAreaSelection(),
  cancelAreaSelection: () => mockCancelAreaSelection(),
  killAreaSelector: () => mockKillAreaSelector(),
}));

vi.mock('@/main/capture/video/recorder.ts', () => ({
  isRecording: () => mockIsRecording(),
  getRecordingDuration: () => mockGetRecordingDuration(),
  getCurrentRecordingPath: () => mockGetCurrentRecordingPath(),
  startRecordingWithConfig: (...a: unknown[]) =>
    mockStartRecordingWithConfig(...a),
  stopRecording: (...a: unknown[]) => mockStopRecording(...a),
  createRecordingProject: () => mockCreateRecordingProject(),
  prewarmRecorder: () => mockPrewarmRecorder(),
}));

vi.mock('@/main/capture/video/overlay.ts', () => ({
  prewarmOverlay: () => mockPrewarmOverlay(),
}));

vi.mock('@/main/capture/video/video-editor.ts', () => ({
  createVideoEditorWindow: (...a: unknown[]) =>
    mockCreateVideoEditorWindow(...a),
}));

vi.mock('@/main/capture/capture-preview', () => ({
  showCapturePreview: (...a: unknown[]) => mockShowCapturePreview(...a),
  prepareCapturePreview: () => mockPrepareCapturePreview(),
  prewarmCapturePreview: () => mockPrewarmCapturePreview(),
}));

vi.mock('@/main/history', () => ({
  addToHistory: (...a: unknown[]) => mockAddToHistory(...a),
}));

vi.mock('@/main/capture/video/permissions.ts', () => ({
  showRecordingError: (...a: unknown[]) => mockShowRecordingError(...a),
  checkAndRequestMicrophonePermission: () => mockCheckMic(),
}));

vi.mock('@/main/capture/video/delete-video.ts', () => ({
  deleteVideo: (...a: unknown[]) => mockDeleteVideo(...a),
}));

vi.mock('@/main/settings', () => ({
  getConfig: () => mockGetConfig(),
  updateConfig: (...a: unknown[]) => mockUpdateConfig(...a),
}));

vi.mock('@/main/capture/video/auto-zoom-generator.ts', () => ({
  generateInitialEditorState: (...a: unknown[]) =>
    mockGenerateInitialEditorState(...a),
}));

describe('recording-actions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockIsRecording.mockReturnValue(false);
    mockGetConfig.mockReturnValue({
      recording: { iosDevice: null, showPreview: false, camera: null },
    });
    mockCreateRecordingProject.mockReturnValue('/p/Rec.capty/recording.mov');
    mockStartRecordingWithConfig.mockResolvedValue(undefined);
    mockEnableCameraProtection.mockResolvedValue(undefined);
    mockDisableCameraProtection.mockResolvedValue(undefined);
    mockHideRecordingControl.mockResolvedValue(undefined);
    mockStopRecording.mockResolvedValue({
      outputPath: '/p/Rec.capty/recording.mov',
      duration: 18,
    });
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockGenerateInitialEditorState.mockResolvedValue(true);
    mockCheckMic.mockResolvedValue(true);
    mockPrepareCapturePreview.mockReturnValue({ prepared: true });
    mockShowCapturePreview.mockReturnValue({ revealed: Promise.resolve() });
  });

  describe('stopRecordingAction', () => {
    it('hides UI and adds to history on success', async () => {
      const m = await import('@/main/capture/video/recording-actions');
      const result = await m.stopRecordingAction();
      expect(mockStopRecording).toHaveBeenCalled();
      expect(mockDisableCameraProtection).toHaveBeenCalled();
      expect(mockHideCameraPreview).toHaveBeenCalled();
      expect(mockHideRecordingControl).toHaveBeenCalled();
      expect(mockAddToHistory).toHaveBeenCalledWith(
        '/p/Rec.capty/recording.mov',
        'video',
        18
      );
      expect(mockCreateVideoEditorWindow).toHaveBeenCalled();
      expect(result).toBe('/p/Rec.capty/recording.mov');
    });

    it('returns null when stop returns no path', async () => {
      mockStopRecording.mockResolvedValue(null);
      const m = await import('@/main/capture/video/recording-actions');
      const result = await m.stopRecordingAction();
      expect(result).toBeNull();
      expect(mockAddToHistory).not.toHaveBeenCalled();
    });

    it('shares finalization across overlapping stop actions', async () => {
      let finishStop: (result: {
        outputPath: string;
        duration: number;
      }) => void = () => {};
      mockStopRecording.mockReturnValueOnce(
        new Promise(resolve => {
          finishStop = resolve;
        })
      );
      const m = await import('@/main/capture/video/recording-actions');

      const firstStop = m.stopRecordingAction();
      const secondStop = m.stopRecordingAction();

      expect(mockStopRecording).toHaveBeenCalledTimes(1);
      finishStop({
        outputPath: '/p/Rec.capty/recording.mov',
        duration: 18,
      });
      await Promise.all([firstStop, secondStop]);

      expect(mockGenerateInitialEditorState).toHaveBeenCalledTimes(1);
      expect(mockAddToHistory).toHaveBeenCalledTimes(1);
      expect(mockCreateVideoEditorWindow).toHaveBeenCalledTimes(1);
    });

    it('waits for a pending start before stopping the recording', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      let finishStartup: () => void = () => {};
      mockStartRecordingWithConfig.mockReturnValueOnce(
        new Promise<void>(resolve => {
          finishStartup = resolve;
        })
      );
      const m = await import('@/main/capture/video/recording-actions');

      const startup = m.startPendingRecording();
      await vi.waitFor(() =>
        expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1)
      );
      const stopping = m.stopRecordingAction();

      expect(mockStopRecording).not.toHaveBeenCalled();
      finishStartup();
      await Promise.all([startup, stopping]);

      expect(mockStopRecording).toHaveBeenCalledTimes(1);
      expect(mockAddToHistory).toHaveBeenCalledTimes(1);
    });

    it('ignores delete while stop finalization is active', async () => {
      let finishStop: (result: {
        outputPath: string;
        duration: number;
      }) => void = () => {};
      mockStopRecording.mockReturnValueOnce(
        new Promise(resolve => {
          finishStop = resolve;
        })
      );
      mockGetCurrentRecordingPath.mockReturnValue('/p/Rec.capty/recording.mov');
      const m = await import('@/main/capture/video/recording-actions');

      const stopping = m.stopRecordingAction();
      await m.deleteRecordingAction();

      expect(mockDeleteVideo).not.toHaveBeenCalled();
      finishStop({
        outputPath: '/p/Rec.capty/recording.mov',
        duration: 18,
      });
      await stopping;

      expect(mockAddToHistory).toHaveBeenCalledTimes(1);
      expect(mockDeleteVideo).not.toHaveBeenCalled();
    });

    it('shows capture preview when configured', async () => {
      mockGetConfig.mockReturnValue({
        recording: { iosDevice: null, showPreview: true, camera: null },
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.stopRecordingAction();
      expect(mockShowCapturePreview).toHaveBeenCalled();
      expect(mockCreateVideoEditorWindow).not.toHaveBeenCalled();
    });

    it('shows the preview without waiting for editor state generation', async () => {
      mockGetConfig.mockReturnValue({
        recording: { iosDevice: null, showPreview: true, camera: null },
      });
      let finishEditorState: (result: boolean) => void = () => {};
      const editorStatePromise = new Promise<boolean>(resolve => {
        finishEditorState = resolve;
      });
      mockGenerateInitialEditorState.mockReturnValueOnce(editorStatePromise);
      const m = await import('@/main/capture/video/recording-actions');

      const stopping = m.stopRecordingAction();

      await vi.waitFor(() => expect(mockShowCapturePreview).toHaveBeenCalled());
      expect(mockShowCapturePreview.mock.calls[0][5]).toBe(editorStatePromise);

      finishEditorState(true);
      await stopping;
    });

    it('shows the preview before starting history persistence', async () => {
      mockGetConfig.mockReturnValue({
        recording: { iosDevice: null, showPreview: true, camera: null },
      });
      let revealPreview: () => void = () => {};
      mockShowCapturePreview.mockReturnValueOnce({
        revealed: new Promise<void>(resolve => {
          revealPreview = resolve;
        }),
      });
      const m = await import('@/main/capture/video/recording-actions');

      const stopping = m.stopRecordingAction();

      await vi.waitFor(() => expect(mockShowCapturePreview).toHaveBeenCalled());
      expect(mockPrepareCapturePreview).toHaveBeenCalled();
      expect(mockAddToHistory).not.toHaveBeenCalled();

      revealPreview();
      await stopping;

      expect(mockAddToHistory).toHaveBeenCalledWith(
        '/p/Rec.capty/recording.mov',
        'video',
        18
      );
    });

    it('falls back to the editor when preview creation fails', async () => {
      mockGetConfig.mockReturnValue({
        recording: { iosDevice: null, showPreview: true, camera: null },
      });
      const dispose = vi.fn();
      mockPrepareCapturePreview.mockReturnValueOnce({ dispose });
      mockShowCapturePreview.mockImplementationOnce(() => {
        throw new Error('preview failed');
      });
      const consoleError = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});
      const m = await import('@/main/capture/video/recording-actions');

      const result = await m.stopRecordingAction();

      expect(dispose).toHaveBeenCalledTimes(1);
      expect(mockCreateVideoEditorWindow).toHaveBeenCalledWith(
        '/p/Rec.capty/recording.mov'
      );
      expect(mockAddToHistory).toHaveBeenCalledWith(
        '/p/Rec.capty/recording.mov',
        'video',
        18
      );
      expect(result).toBe('/p/Rec.capty/recording.mov');
      consoleError.mockRestore();
    });

    it('queues a new recording until previous finalization finishes', async () => {
      let showPreview = true;
      mockGetConfig.mockImplementation(() => ({
        recording: { iosDevice: null, showPreview, camera: null },
      }));
      let revealPreview: () => void = () => {};
      mockShowCapturePreview.mockReturnValueOnce({
        revealed: new Promise<void>(resolve => {
          revealPreview = resolve;
        }),
      });
      const m = await import('@/main/capture/video/recording-actions');

      const firstStop = m.stopRecordingAction();
      await vi.waitFor(() => expect(mockShowCapturePreview).toHaveBeenCalled());

      showPreview = false;
      const nextStart = m.startPendingRecording({
        iosDeviceId: 'ios-device',
        iosDeviceName: 'iPhone',
      });
      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
      revealPreview();
      await Promise.all([firstStop, nextStart]);

      mockStopRecording.mockResolvedValueOnce({
        outputPath: '/p/Second.capty/recording.mov',
        duration: 10,
      });
      await m.stopRecordingAction();

      expect(mockGenerateInitialEditorState).toHaveBeenLastCalledWith({
        projectPath: '/p/Second.capty/recording.mov',
        recordingType: 'ios-device',
      });
    });

    it('cleans up even on stop error', async () => {
      mockStopRecording.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/video/recording-actions');
      await expect(m.stopRecordingAction()).rejects.toThrow('boom');
      expect(mockHideRecordingControl).toHaveBeenCalled();
    });
  });

  describe('recordArea', () => {
    it('does nothing when already recording', async () => {
      mockIsRecording.mockReturnValue(true);
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordArea();
      expect(mockStartAreaSelection).not.toHaveBeenCalled();
    });

    it('starts area selection and pre-warms', async () => {
      mockStartAreaSelection.mockResolvedValue({ status: 'confirmed' });
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordArea();
      expect(mockPrewarmRecordingControl).toHaveBeenCalled();
      expect(mockPrewarmRecorder).toHaveBeenCalled();
      expect(mockPrewarmOverlay).toHaveBeenCalled();
      expect(mockStartAreaSelection).toHaveBeenCalled();
    });

    it('onSelected shows pre-recording control with bounds', async () => {
      mockStartAreaSelection.mockImplementation(
        async (opts: { onSelected: (s: unknown) => void }) => {
          opts.onSelected({
            status: 'selected',
            x: 10,
            y: 20,
            width: 100,
            height: 200,
          });
        }
      );
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordArea();
      expect(mockShowPreRecordingControl).toHaveBeenCalledWith({
        x: 10,
        y: 20,
        width: 100,
        height: 200,
      });
    });

    it('onUpdate updates control position', async () => {
      mockStartAreaSelection.mockImplementation(
        async (opts: { onUpdate: (s: unknown) => void }) => {
          opts.onUpdate({
            status: 'updated',
            x: 50,
            y: 60,
            width: 100,
            height: 100,
          });
        }
      );
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordArea();
      expect(mockUpdatePosition).toHaveBeenCalled();
    });

    it('onCancelled hides pre-recording control', async () => {
      mockStartAreaSelection.mockImplementation(
        async (opts: { onCancelled: () => void }) => {
          opts.onCancelled();
        }
      );
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordArea();
      expect(mockHidePreRecordingControl).toHaveBeenCalled();
    });

    it('swallows daemon errors', async () => {
      mockStartAreaSelection.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/video/recording-actions');
      await expect(m.recordArea()).resolves.toBeUndefined();
    });

    it('clears iosDevice config', async () => {
      mockGetConfig.mockReturnValue({
        recording: {
          iosDevice: { id: 'ios-1', name: 'My iPhone' },
          camera: null,
        },
      });
      mockStartAreaSelection.mockResolvedValue(null);
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordArea();
      expect(mockUpdateConfig).toHaveBeenCalled();
    });
  });

  describe('recordScreen', () => {
    it('starts area selection in display mode', async () => {
      mockStartAreaSelection.mockResolvedValue(null);
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordScreen();
      const [opts] = mockStartAreaSelection.mock.calls[0];
      expect(opts.mode).toBe('display');
    });

    it('skips when already recording', async () => {
      mockIsRecording.mockReturnValue(true);
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordScreen();
      expect(mockStartAreaSelection).not.toHaveBeenCalled();
    });

    it('swallows errors', async () => {
      mockStartAreaSelection.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/video/recording-actions');
      await expect(m.recordScreen()).resolves.toBeUndefined();
    });

    it('onSelected, onUpdate, onCancelled all wire up', async () => {
      mockStartAreaSelection.mockImplementation(
        async (opts: {
          onSelected: (s: unknown) => void;
          onUpdate: (s: unknown) => void;
          onCancelled: () => void;
        }) => {
          opts.onSelected({
            status: 'selected',
            x: 1,
            y: 2,
            width: 3,
            height: 4,
          });
          opts.onUpdate({
            status: 'updated',
            x: 5,
            y: 6,
            width: 7,
            height: 8,
          });
          opts.onCancelled();
        }
      );
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordScreen();
      expect(mockShowPreRecordingControl).toHaveBeenCalled();
      expect(mockUpdatePosition).toHaveBeenCalled();
      expect(mockHidePreRecordingControl).toHaveBeenCalled();
    });
  });

  describe('recordWindow', () => {
    it('starts area selection in window mode', async () => {
      mockStartAreaSelection.mockResolvedValue(null);
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordWindow();
      const [opts] = mockStartAreaSelection.mock.calls[0];
      expect(opts.mode).toBe('window');
    });

    it('skips when already recording', async () => {
      mockIsRecording.mockReturnValue(true);
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordWindow();
      expect(mockStartAreaSelection).not.toHaveBeenCalled();
    });

    it('swallows errors', async () => {
      mockStartAreaSelection.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/video/recording-actions');
      await expect(m.recordWindow()).resolves.toBeUndefined();
    });

    it('onSelected, onUpdate, onCancelled all wire up', async () => {
      mockStartAreaSelection.mockImplementation(
        async (opts: {
          onSelected: (s: unknown) => void;
          onUpdate: (s: unknown) => void;
          onCancelled: () => void;
        }) => {
          opts.onSelected({
            status: 'selected',
            x: 10,
            y: 20,
            width: 30,
            height: 40,
          });
          opts.onUpdate({
            status: 'updated',
            x: 50,
            y: 60,
            width: 70,
            height: 80,
          });
          opts.onCancelled();
        }
      );
      const m = await import('@/main/capture/video/recording-actions');
      await m.recordWindow();
      expect(mockShowPreRecordingControl).toHaveBeenCalled();
      expect(mockUpdatePosition).toHaveBeenCalled();
      expect(mockHidePreRecordingControl).toHaveBeenCalled();
    });
  });

  describe('startPendingRecording', () => {
    it('returns when no selection confirmed', async () => {
      mockConfirmAreaSelection.mockResolvedValue(null);
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
    });

    it('returns when selection is cancelled', async () => {
      mockConfirmAreaSelection.mockResolvedValue({ status: 'cancelled' });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
    });

    it('returns when selection has no bounds', async () => {
      mockConfirmAreaSelection.mockResolvedValue({ status: 'selected' });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
    });

    it('starts recording for area capture', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      expect(mockStartRecordingWithConfig).toHaveBeenCalled();
    });

    it('shares startup across duplicate recording requests', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      let finishStartup: () => void = () => {};
      mockStartRecordingWithConfig.mockReturnValueOnce(
        new Promise<void>(resolve => {
          finishStartup = resolve;
        })
      );
      const m = await import('@/main/capture/video/recording-actions');

      const first = m.startPendingRecording({ cameraEnabled: true });
      await vi.waitFor(() =>
        expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1)
      );
      const second = m.startPendingRecording({ cameraEnabled: true });

      expect(mockConfirmAreaSelection).toHaveBeenCalledTimes(1);
      expect(mockCreateRecordingProject).toHaveBeenCalledTimes(1);
      expect(mockDisableCameraProtection).not.toHaveBeenCalled();
      expect(mockHideCameraPreview).not.toHaveBeenCalled();

      finishStartup();
      await Promise.all([first, second]);

      expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1);
    });

    it('iOS recording skips area selector', async () => {
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording({
        iosDeviceId: 'ios-1',
        iosDeviceName: 'iPhone',
      });
      expect(mockKillAreaSelector).toHaveBeenCalled();
      expect(mockConfirmAreaSelection).not.toHaveBeenCalled();
      expect(mockStartRecordingWithConfig).toHaveBeenCalled();
    });

    it('preserves the pending selection when mic permission is denied', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      mockCheckMic.mockResolvedValue(false);
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording({ micEnabled: true });
      expect(mockConfirmAreaSelection).not.toHaveBeenCalled();
      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();

      mockCheckMic.mockResolvedValue(true);
      await m.startPendingRecording({ micEnabled: true });
      expect(mockConfirmAreaSelection).toHaveBeenCalledTimes(1);
      expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1);
    });

    it('enables camera content protection when camera enabled', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording({ cameraEnabled: true });
      expect(mockEnableCameraProtection).toHaveBeenCalled();
      expect(
        mockEnableCameraProtection.mock.invocationCallOrder[0]
      ).toBeLessThan(mockStartRecordingWithConfig.mock.invocationCallOrder[0]);
    });

    it('does not start when camera content protection fails', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      mockEnableCameraProtection.mockRejectedValue(
        new Error('protection failed')
      );
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording({ cameraEnabled: true });
      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
      expect(mockShowRecordingError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'protection failed' })
      );
    });

    it('waits for native startup before restarting', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      let finishStartup: () => void = () => {};
      mockStartRecordingWithConfig.mockReturnValueOnce(
        new Promise<void>(resolve => {
          finishStartup = resolve;
        })
      );
      const m = await import('@/main/capture/video/recording-actions');
      const startup = m.startPendingRecording();
      await vi.waitFor(() =>
        expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1)
      );

      const restarting = m.restartRecordingAction();

      expect(mockStopRecording).not.toHaveBeenCalled();
      finishStartup();
      await Promise.all([startup, restarting]);

      expect(mockStopRecording).toHaveBeenCalledTimes(1);
      expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(2);
    });

    it('wires post-start recording failure cleanup', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording({ cameraEnabled: true });
      const onFailure = mockStartRecordingWithConfig.mock.calls[0][3] as (
        error: Error,
        outputPath: string | null
      ) => Promise<void>;

      await onFailure(
        new Error('capture failed'),
        '/p/Rec.capty/recording.mov'
      );

      expect(mockDisableCameraProtection).toHaveBeenCalled();
      expect(mockHideRecordingControl).toHaveBeenCalled();
      expect(mockHideCameraPreview).toHaveBeenCalled();
      expect(mockDeleteVideo).toHaveBeenCalledWith(
        '/p/Rec.capty/recording.mov',
        {
          showNotification: false,
          showErrorDialog: false,
        }
      );
      expect(mockShowRecordingError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'capture failed' })
      );
    });

    it('shows error and cleans up on start failure', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      mockStartRecordingWithConfig.mockRejectedValue(new Error('boom'));
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      expect(mockShowRecordingError).toHaveBeenCalled();
      expect(mockDisableCameraProtection).toHaveBeenCalled();
      expect(mockDeleteVideo).toHaveBeenCalledWith(
        '/p/Rec.capty/recording.mov',
        {
          showNotification: false,
          showErrorDialog: false,
        }
      );
    });

    it('shows error when the recording project cannot be created', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      mockCreateRecordingProject.mockImplementationOnce(() => {
        throw new Error('disk full');
      });
      const m = await import('@/main/capture/video/recording-actions');

      await m.startPendingRecording();

      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
      expect(mockShowRecordingError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'disk full' })
      );
      expect(mockDeleteVideo).not.toHaveBeenCalled();
      expect(mockDisableCameraProtection).toHaveBeenCalled();
    });

    it('silently cleans up when recording startup is cancelled', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const cancellation = new Error('Recording start cancelled');
      cancellation.name = 'AbortError';
      mockStartRecordingWithConfig.mockRejectedValueOnce(cancellation);
      const m = await import('@/main/capture/video/recording-actions');

      await m.startPendingRecording();

      expect(mockShowRecordingError).not.toHaveBeenCalled();
      expect(mockDeleteVideo).toHaveBeenCalledWith(
        '/p/Rec.capty/recording.mov',
        {
          showNotification: false,
          showErrorDialog: false,
        }
      );
    });
  });

  describe('cancelPendingRecording', () => {
    it('cancels area selection and hides control', async () => {
      const m = await import('@/main/capture/video/recording-actions');
      await m.cancelPendingRecording();
      expect(mockCancelAreaSelection).toHaveBeenCalled();
      expect(mockHidePreRecordingControl).toHaveBeenCalled();
    });
  });

  describe('deleteRecordingAction', () => {
    it('stops and deletes the current recording', async () => {
      mockGetCurrentRecordingPath.mockReturnValue('/p/x.mov');
      const m = await import('@/main/capture/video/recording-actions');
      await m.deleteRecordingAction();
      expect(mockStopRecording).toHaveBeenCalled();
      expect(mockDeleteVideo).toHaveBeenCalledWith('/p/x.mov', {
        showNotification: true,
      });
    });

    it('skips delete when no path', async () => {
      mockGetCurrentRecordingPath.mockReturnValue(null);
      const m = await import('@/main/capture/video/recording-actions');
      await m.deleteRecordingAction();
      expect(mockDeleteVideo).not.toHaveBeenCalled();
    });

    it('cleans up even on stop error', async () => {
      mockStopRecording.mockRejectedValue(new Error('boom'));
      mockGetCurrentRecordingPath.mockReturnValue('/p/x.mov');
      const m = await import('@/main/capture/video/recording-actions');
      await expect(m.deleteRecordingAction()).rejects.toThrow('boom');
      expect(mockHideRecordingControl).toHaveBeenCalled();
      expect(mockDeleteVideo).toHaveBeenCalledWith('/p/x.mov', {
        showNotification: true,
      });
    });
  });

  describe('restartRecordingAction', () => {
    it('does nothing when no previous config', async () => {
      const m = await import('@/main/capture/video/recording-actions');
      await m.restartRecordingAction();
      expect(mockStopRecording).not.toHaveBeenCalled();
    });

    it('restarts with previous config', async () => {
      // Bootstrap a previous config via startPendingRecording
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      mockStartRecordingWithConfig.mockClear();
      mockGetCurrentRecordingPath.mockReturnValue('/p/old.mov');
      await m.restartRecordingAction();
      expect(mockStopRecording).toHaveBeenCalled();
      expect(mockDeleteVideo).toHaveBeenCalledWith('/p/old.mov', {
        showNotification: false,
      });
      expect(mockStartRecordingWithConfig).toHaveBeenCalled();
    });

    it('shares restart work across overlapping requests', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      mockCreateRecordingProject.mockClear();
      mockStartRecordingWithConfig.mockClear();
      let finishStop: () => void = () => {};
      mockStopRecording.mockReturnValueOnce(
        new Promise(resolve => {
          finishStop = () =>
            resolve({
              outputPath: '/p/old.mov',
              duration: 10,
            });
        })
      );
      mockGetCurrentRecordingPath.mockReturnValue('/p/old.mov');

      const firstRestart = m.restartRecordingAction();
      const secondRestart = m.restartRecordingAction();

      expect(mockStopRecording).toHaveBeenCalledTimes(1);
      finishStop();
      await Promise.all([firstRestart, secondRestart]);

      expect(mockCreateRecordingProject).toHaveBeenCalledTimes(1);
      expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1);
      expect(mockDeleteVideo).toHaveBeenCalledTimes(1);
    });

    it('does not start another recording while restart is pending', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      mockStartRecordingWithConfig.mockClear();
      let finishStop: () => void = () => {};
      mockStopRecording.mockReturnValueOnce(
        new Promise(resolve => {
          finishStop = () =>
            resolve({
              outputPath: '/p/old.mov',
              duration: 10,
            });
        })
      );
      mockGetCurrentRecordingPath.mockReturnValue('/p/old.mov');

      const restarting = m.restartRecordingAction();
      const overlappingStart = m.startPendingRecording({
        iosDeviceId: 'ios-device',
        iosDeviceName: 'iPhone',
      });

      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
      finishStop();
      await Promise.all([restarting, overlappingStart]);

      expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1);
      expect(mockKillAreaSelector).not.toHaveBeenCalled();
    });

    it('shows error on restart failure', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      mockStartRecordingWithConfig.mockRejectedValueOnce(new Error('boom'));
      await m.restartRecordingAction();
      expect(mockShowRecordingError).toHaveBeenCalled();
      expect(mockDeleteVideo).toHaveBeenLastCalledWith(
        '/p/Rec.capty/recording.mov',
        {
          showNotification: false,
          showErrorDialog: false,
        }
      );
    });

    it('shows error when a restart project cannot be created', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording();
      mockCreateRecordingProject.mockImplementationOnce(() => {
        throw new Error('disk full');
      });

      await m.restartRecordingAction();

      expect(mockShowRecordingError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'disk full' })
      );
      expect(mockStartRecordingWithConfig).toHaveBeenCalledTimes(1);
    });

    it('shows camera preview when camera enabled in previous config', async () => {
      mockConfirmAreaSelection.mockResolvedValue({
        status: 'selected',
        x: 0,
        y: 0,
        width: 100,
        height: 100,
      });
      mockGetConfig.mockReturnValue({
        recording: {
          iosDevice: null,
          camera: { enabled: true, selectedDeviceId: 'cam-1' },
        },
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.startPendingRecording({ cameraEnabled: true });
      mockShowCameraPreview.mockClear();
      await m.restartRecordingAction();
      expect(mockShowCameraPreview).toHaveBeenCalled();
    });
  });
});
