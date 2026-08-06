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
    mockStopRecording.mockResolvedValue('/p/Rec.capty/recording.mov');
    mockAddToHistory.mockResolvedValue({ id: 'h1' });
    mockGenerateInitialEditorState.mockResolvedValue(true);
    mockCheckMic.mockResolvedValue(true);
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
        5
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

    it('shows capture preview when configured', async () => {
      mockGetConfig.mockReturnValue({
        recording: { iosDevice: null, showPreview: true, camera: null },
      });
      const m = await import('@/main/capture/video/recording-actions');
      await m.stopRecordingAction();
      expect(mockShowCapturePreview).toHaveBeenCalled();
      expect(mockCreateVideoEditorWindow).not.toHaveBeenCalled();
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

    it('aborts when mic permission denied', async () => {
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
      expect(mockStartRecordingWithConfig).not.toHaveBeenCalled();
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
